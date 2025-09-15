//! RPC message framing and transmission as specified in RFC 5531 (previously RFC 1057 section 10).
//!
//! This module implements the Record Marking Standard for sending RPC messages
//! over TCP connections. It provides:
//!
//! - Message fragmentation for large RPC messages
//! - Proper message delimitation in stream-oriented transports
//! - Asynchronous message processing
//! - RPC call dispatching to appropriate protocol handlers
//!
//! The wire protocol implementation handles all the low-level details of:
//! - Reading fragmentary messages and reassembling them
//! - Writing record-marked fragments with appropriate headers
//! - Managing socket communication channels
//! - Processing incoming RPC calls
//!
//! This module is essential for maintaining proper message boundaries in TCP
//! while providing efficient transmission of RPC messages of any size.

use std::io;
use std::io::Cursor;
use std::io::{Read, Write};

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::{ReadHalf, SimplexStream, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, trace, warn};

use crate::protocol::xdr::{self, deserialize, mount, nfs3, portmap, Serialize};
use crate::protocol::{nfs, rpc};
use crate::tcp::{CommandResult, ResponseBuffer, RpcCommand};
use crate::utils::error::io_other;

// Information from RFC 5531 (ONC RPC v2)
// https://datatracker.ietf.org/doc/html/rfc5531
// Which obsoletes RFC 1831 and RFC 1057 (Original RPC)

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
    mut context: rpc::Context,
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
                nfs3::VERSION => nfs::v3::handle_nfs(xid, call, input, output, &context).await,
                nfs::v4::VERSION => nfs::v4::handle_nfs(xid, call, input, output, &context).await,
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
                nfs::portmap::handle_portmap(xid, &call, input, output, &mut context).await
            }
            mount::PROGRAM => nfs::mount::handle_mount(xid, call, input, output, &context).await,
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

/// Handles RPC message processing over a TCP connection
///
/// Receives record-marked RPC messages from a TCP stream, processes
/// them asynchronously by dispatching to the appropriate protocol handlers,
/// and manages the response flow. Implements the record marking protocol
/// for reliable message delimitation over TCP.
#[derive(Debug)]
pub struct SocketMessageHandler {
    /// Command queue for ordered processing
    pub command_queue: UnboundedSender<RpcCommand>,
}

impl SocketMessageHandler {
    /// Creates a new `SocketMessageHandler` instance
    ///
    /// Initializes the handler with the provided RPC context and creates the
    /// necessary communication channels. Returns the handler itself, a duplex
    /// stream for writing to the socket, and a receiver for processed messages.
    ///
    /// This setup enables asynchronous processing of RPC messages while maintaining
    /// order of operations.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<CommandResult>) {
        // Create separate channel for command results
        let (result_sender, mut result_receiver) = mpsc::unbounded_channel::<CommandResult>();
        let (command_sender, mut command_receiver) = mpsc::unbounded_channel::<RpcCommand>();

        // Start worker task that processes commands in order
        tokio::spawn(async move {
            // Create reusable buffer for responses
            let mut output_buffer = ResponseBuffer::with_capacity(DEFAULT_RESPONSE_BUFFER_CAPACITY);

            while let Some(command) = command_receiver.recv().await {
                trace!("Processing command from queue");

                // Clear buffer for reuse
                output_buffer.clear();

                // Call async processor
                let result =
                    match process_rpc_command(&command.data, &mut output_buffer, command.context).await {
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


        (
            Self {
                command_queue: command_sender,
            },
            result_receiver,
        )
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
pub fn process_rpc_command<'a>(
    data: &[u8],
    output: &'a mut ResponseBuffer,
    context: rpc::Context,
) -> futures::future::BoxFuture<'a, io::Result<bool>> {
    // Clone data to own it in closure
    let data_clone = data.to_vec();

    Box::pin(async move {
        // Create cursor for reading data
        let mut input_cursor = Cursor::new(data_clone);

        // Get internal buffer for writing
        let output_buffer = output.get_mut_buffer();
        let mut output_cursor = Cursor::new(output_buffer);

        // Call RPC handler
        let result = handle_rpc(&mut input_cursor, &mut output_cursor, context).await?;

        // If response was generated, return true
        Ok(result)
    })
}
