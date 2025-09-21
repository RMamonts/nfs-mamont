use std::io;
use std::io::{Cursor, Read, Write};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error, trace, warn};

use crate::protocol::nfs;
use crate::protocol::rpc::Context;
use crate::response_buffer::ResponseBuffer;
use crate::rpc_command::RpcCommand;
use crate::tcp::CommandResult;
use crate::utils::error::io_other;
use crate::xdr;
use crate::xdr::{deserialize, mount, nfs3, portmap, Serialize};

/// RPC program number for NFS Access Control Lists
const NFS_ACL_PROGRAM: u32 = 100227;
/// RPC program number for NFS ID Mapping
const NFS_ID_MAP_PROGRAM: u32 = 100270;
/// RPC program number for LOCALIO auxiliary RPC protocol
/// More about <https://docs.kernel.org/filesystems/nfs/localio.html#rpc.>
const NFS_LOCALIO_PROGRAM: u32 = 400122;
/// RPC program number for NFS Metadata
const NFS_METADATA_PROGRAM: u32 = 200024;
/// Initial size of RPC response buffer
const DEFAULT_RESPONSE_BUFFER_CAPACITY: usize = 8192;

/// Task responsible for handling Virtual File System (VFS) operations asynchronously.
///
/// The `VfsTask` typically manages file system operations, directory traversals,
/// file metadata queries, and other VFS-related functionality in a non-blocking manner.
/// It usually communicates with other components through channels to receive requests
/// and send responses.
pub struct VfsTask;

impl VfsTask {
    /// Spawns an asynchronous task that processes RPC commands from a queue.
    ///
    /// This task continuously listens for [`RpcCommand`] messages from an unbounded channel,
    /// processes them asynchronously, and sends the results back through another channel.
    /// It efficiently reuses response buffers to minimize memory allocations.
    ///
    /// # Parameters
    /// - `command_receiver`: An [`UnboundedReceiver`] that receives [`RpcCommand`] messages
    ///   to be processed. The task will process commands in the order they are received.
    /// - `result_sender`: An [`UnboundedSender`] used to send [`CommandResult`] messages back
    ///   to the result handler (typically a [`WriteTask`]).
    /// - `context`: A [`Context`] instance containing shared state, configuration, or resources
    ///   needed for command processing. This is passed mutably to each command processor.
    ///
    /// # Behavior
    /// - **Command Processing**: Each command is processed asynchronously by [`process_rpc_command`]
    /// - **Buffer Reuse**: A reusable [`ResponseBuffer`] is maintained to minimize allocations
    /// - **Result Routing**: Results are sent back through the result channel for writing
    /// - **Error Handling**: Processing errors are propagated as [`Err`] results
    /// - **Graceful Shutdown**: Task terminates when the command channel is closed
    ///
    /// # Response Handling
    /// - **`Ok(true)`**: Processor indicates response should be sent (marks buffer as having content)
    /// - **`Ok(false)`**: No response needed (e.g., retransmissions, acknowledgments)
    /// - **`Err(e)`**: Processing error occurred, propagated to result handler
    pub fn spawn(
        mut command_receiver: UnboundedReceiver<RpcCommand>,
        result_sender: UnboundedSender<CommandResult>,
        mut context: Context,
    ) {
        tokio::spawn(async move {
            // Create reusable buffer for responses
            let mut output_buffer = ResponseBuffer::with_capacity(DEFAULT_RESPONSE_BUFFER_CAPACITY);

            while let Some(command) = command_receiver.recv().await {
                trace!("Processing command from queue");

                // Clear buffer for reuse
                output_buffer.clear();
                // Call async processor
                let result =
                    match process_rpc_command(command.data, &mut output_buffer, &mut context).await
                    {
                        Ok(true) => {
                            // Processor indicated response needs to be sent
                            output_buffer.mark_has_content();
                            let buffer_to_send = std::mem::replace(
                                &mut output_buffer,
                                ResponseBuffer::with_capacity(DEFAULT_RESPONSE_BUFFER_CAPACITY),
                            );
                            Ok(Some(buffer_to_send))
                        }
                        Ok(false) => {
                            // No response needed (e.g. retransmission)
                            Ok(None)
                        }
                        Err(e) => Err(e),
                    };

                // Send result
                if let Err(e) = result_sender.send(result) {
                    error!("Failed to send command processing result: {:?}", e);
                    break;
                }
            }
            debug!("Command queue handler finished");
        });
    }
}

/// Processes a single RPC message
///
/// This function forms the core of the RPC message dispatcher. It:
/// 1. Deserializes the incoming RPC message using XDR format
/// 2. Validates the RPC version number (must be version 2)
/// 3. Extracts authentication information if provided
/// 4. Checks for retransmissions to ensure idempotent operation
/// 5. Routes the call to the appropriate protocol handler (NFS, MOUNT, PORTMAP)
/// 6. Tracks transaction completion state
///
/// This implementation follows RFC 5531 (previously RFC 1057) section on Authentication and
/// Record Marking Standard for proper RPC message handling.
///
/// Returns true if a response was sent, false otherwise (for retransmissions).
pub async fn handle_rpc(
    input: &mut impl Read,
    output: &mut impl Write,
    context: &mut Context,
) -> io::Result<bool> {
    let recv = deserialize::<xdr::rpc::rpc_msg>(input)?;
    let xid = recv.xid;
    if let xdr::rpc::rpc_body::CALL(call) = recv.body {
        if let xdr::rpc::auth_flavor::AUTH_SYS = call.cred.flavor {
            let auth = deserialize(&mut Cursor::new(&call.cred.body))?;
            context.auth = Some(auth);
        }
        if call.rpcvers != xdr::rpc::PROTOCOL_VERSION {
            warn!("Invalid RPC version {} != 2", call.rpcvers);
            xdr::rpc::rpc_vers_mismatch(xid).serialize(output)?;
            return Ok(true);
        }

        if context.transaction_tracker.is_retransmission(xid, &context.client_addr) {
            debug!(
                "Retransmission detected, xid: {}, client_addr: {}, call: {:?}",
                xid, context.client_addr, call
            );
            return Ok(false);
        }

        let result = match call.prog {
            nfs3::PROGRAM => match call.vers {
                nfs3::VERSION => nfs::v3::handle_nfs(xid, call, input, output, context).await,
                nfs::v4::VERSION => nfs::v4::handle_nfs(xid, call, input, output, context).await,
                v => {
                    warn!("Unsupported NFS version: {}", v);
                    xdr::rpc::prog_version_range_mismatch_reply_message(
                        xid,
                        nfs3::VERSION,
                        nfs::v4::VERSION,
                    )
                    .serialize(output)?;
                    Ok(())
                }
            },
            portmap::PROGRAM => {
                nfs::portmap::handle_portmap(xid, &call, input, output, context).await
            }
            mount::PROGRAM => nfs::mount::handle_mount(xid, call, input, output, context).await,
            prog if prog == NFS_ACL_PROGRAM
                || prog == NFS_ID_MAP_PROGRAM
                || prog == NFS_METADATA_PROGRAM =>
            {
                trace!("ignoring NFS_ACL/ID_MAP/METADATA packet");
                xdr::rpc::prog_unavail_reply_message(xid).serialize(output)?;
                Ok(())
            }
            NFS_LOCALIO_PROGRAM => {
                trace!("Ignoring NFS_LOCALIO packet");
                xdr::rpc::prog_unavail_reply_message(xid).serialize(output)?;
                Ok(())
            }
            _ => {
                warn!("Unknown RPC Program number {} != {}", call.prog, nfs3::PROGRAM);
                xdr::rpc::prog_unavail_reply_message(xid).serialize(output)?;
                Ok(())
            }
        }
        .map(|_| true);

        context.transaction_tracker.mark_processed(xid, &context.client_addr);

        result
    } else {
        error!("Unexpectedly received a Reply instead of a Call");
        io_other("Bad RPC Call format")
    }
}

/// Standard async RPC processing function that can be used with `CommandQueue`
///
/// Processes an RPC command by:
/// 1. Deserializing the RPC message
/// 2. Processing the RPC call according to standard protocol
/// 3. Writing response to output buffer
///
/// # Arguments
///
/// * `data` - Buffer containing RPC message
/// * `output` - Buffer for writing response
/// * `context` - RPC processing context
///
/// # Returns
///
/// `Ok(true)` if response needs to be sent
/// `Ok(false)` if no response needed (e.g. retransmission)
/// `Err` if processing error occurred
pub async fn process_rpc_command(
    data: Vec<u8>,
    output: &mut ResponseBuffer,
    context: &mut Context,
) -> io::Result<bool> {
    // Create cursor for reading data
    let mut input_cursor = Cursor::new(data);

    // Get internal buffer for writing
    let output_buffer = output.get_mut_buffer();
    let mut output_cursor = Cursor::new(output_buffer);

    // Call RPC handler
    let result = handle_rpc(&mut input_cursor, &mut output_cursor, context).await?;

    // If response was generated, return true
    Ok(result)
}
