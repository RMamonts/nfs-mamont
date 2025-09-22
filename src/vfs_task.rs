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
pub struct VfsTask {
    /// Channel receiver for incoming RPC commands to process
    command_receiver: UnboundedReceiver<RpcCommand>,
    /// Channel sender for sending command processing results back to requester
    result_sender: UnboundedSender<CommandResult>,
    /// Shared context containing VFS state and configuration
    context: Context,
}

impl VfsTask {
    /// Creates new instance of [`VfsTask`]
    pub fn new(
        command_receiver: UnboundedReceiver<RpcCommand>,
        result_sender: UnboundedSender<CommandResult>,
        context: Context,
    ) -> Self {
        Self { command_receiver, result_sender, context }
    }

    /// Spawns a background task that processes VFS operations from a command queue.
    ///
    /// This method moves ownership of the instance to a new Tokio task that will
    /// call method [`run()`](#method.run) to process VFS operations.
    ///
    /// # Panics
    ///
    /// This method does not panic. Any errors encountered during task execution
    /// are properly logged and the task exits cleanly.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    /// Main function to process VFS RPC commands from the queue.
    ///
    /// This method runs a loop that:
    /// 1. Receives commands from the command channel
    /// 2. Processes each command using the VFS handler
    /// 3. Manages response buffering efficiently
    /// 4. Sends results back through the result channel
    ///
    /// The loop continues until the command channel is closed or an unrecoverable error occurs.
    async fn run(mut self) {
        // Create reusable buffer for responses
        let mut output_buffer = ResponseBuffer::with_capacity(DEFAULT_RESPONSE_BUFFER_CAPACITY);

        while let Some(command) = self.command_receiver.recv().await {
            trace!("Processing command from queue");

            // Clear buffer for reuse
            output_buffer.clear();

            let mut input_cursor = Cursor::new(command.data);
            let mut output_cursor = Cursor::new(output_buffer.get_mut_buffer());

            // Call async processor
            let result =
                match Self::handle_rpc(&mut input_cursor, &mut output_cursor, &mut self.context)
                    .await
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
            if let Err(e) = self.result_sender.send(result) {
                error!("Failed to send command processing result: {:?}", e);
                break;
            }
        }
        debug!("Command queue handler finished");
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
                    nfs::v4::VERSION => {
                        nfs::v4::handle_nfs(xid, call, input, output, context).await
                    }
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
}
