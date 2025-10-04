use std::io::{Cursor, Read, Write};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error, trace, warn};

use crate::protocol::nfs;
use crate::protocol::rpc::Context;
use crate::response_buffer::ResponseBuffer;
use crate::rpc_command::RpcCommand;
use crate::tcp::CommandResult;
use crate::xdr;
use crate::xdr::rpc::{accept_body, mismatch_info, rejected_reply, rpc_msg, PROTOCOL_VERSION};
use crate::xdr::{deserialize, mount, nfs3, ProtocolErrors};

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

/// An asynchronous task responsible for processing RPC commands,
/// and sending operation results - [`ResponseBuffer`] to [`WriteTask`].
pub struct VfsTask {
    command_receiver: UnboundedReceiver<RpcCommand>,
    result_sender: UnboundedSender<CommandResult>,
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

    /// Spawns a [`VfsTask`] that receives command from [`ReadTask`],
    /// processes it and sends results to [`WriteTask`]
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(mut self) {
        // Create reusable buffer for responses
        let mut output_buffer = ResponseBuffer::with_capacity(DEFAULT_RESPONSE_BUFFER_CAPACITY);

        while let Some(command) = self.command_receiver.recv().await {
            trace!("Processing command from queue");

            // Clear buffer for reuse
            output_buffer.clear();

            let mut input_cursor = Cursor::new(command.data);
            let mut output_cursor = Cursor::new(output_buffer.get_mut_buffer());

            if let Ok(recv) = deserialize::<xdr::rpc::rpc_msg>(&mut input_cursor) {
                let xid = recv.xid;
                // Call async processor
                let result =
                    match self.handle_rpc(xid, recv, &mut input_cursor, &mut output_cursor).await {
                        Ok(_) => {
                            // Processor indicated response needs to be sent
                            output_buffer.mark_has_content();
                            let buffer_to_send = std::mem::replace(
                                &mut output_buffer,
                                ResponseBuffer::with_capacity(DEFAULT_RESPONSE_BUFFER_CAPACITY),
                            );
                            Ok(buffer_to_send)
                        }
                        Err(e) => Err(e),
                    };
                // Send result
                if let Err(e) = self.result_sender.send(CommandResult::from((xid, result))) {
                    error!("Failed to send command processing result: {:?}", e);
                    break;
                }
            } else {
                error!("Cannot process REPLY");
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
    /// 4. Routes the call to the appropriate protocol handler (NFS, MOUNT, PORTMAP)
    /// 5. Tracks transaction completion state
    ///
    /// This implementation follows RFC 5531 (previously RFC 1057) section on Authentication and
    /// Record Marking Standard for proper RPC message handling.
    pub async fn handle_rpc(
        &mut self,
        xid: u32,
        recv: rpc_msg,
        input: &mut impl Read,
        output: &mut impl Write,
    ) -> Result<(), ProtocolErrors> {
        if let xdr::rpc::rpc_body::CALL(call) = recv.body {
            if let xdr::rpc::auth_flavor::AUTH_SYS = call.cred.flavor {
                let auth = deserialize(&mut Cursor::new(&call.cred.body))
                    .map_err(|_| ProtocolErrors::RpcAccepted(accept_body::GARBAGE_ARGS))?;
                self.context.auth = Some(auth);
            }
            if call.rpcvers != PROTOCOL_VERSION {
                warn!("Invalid RPC version {} != 2", call.rpcvers);
                return Err(ProtocolErrors::RpcRejected(rejected_reply::RPC_MISMATCH(
                    mismatch_info { low: PROTOCOL_VERSION, high: PROTOCOL_VERSION },
                )));
            }

            match call.prog {
                nfs3::PROGRAM => match call.vers {
                    nfs3::VERSION => {
                        Ok(nfs::v3::handle_nfs(xid, call, input, output, &self.context).await?)
                    }
                    nfs::v4::VERSION => {
                        Ok(nfs::v4::handle_nfs(xid, call, input, output, &self.context).await?)
                    }
                    v => {
                        warn!("Unsupported NFS version: {}", v);
                        Err(ProtocolErrors::RpcAccepted(accept_body::PROG_MISMATCH(
                            mismatch_info { low: nfs3::VERSION, high: nfs::v4::VERSION },
                        )))
                    }
                },
                mount::PROGRAM => match call.vers {
                    mount::VERSION => {
                        Ok(nfs::mount::handle_mount(xid, call, input, output, &self.context)
                            .await?)
                    }
                    v => {
                        warn!("Unsupported Mount version: {}", v);
                        Err(ProtocolErrors::RpcAccepted(accept_body::PROG_MISMATCH(
                            mismatch_info { low: mount::VERSION, high: mount::VERSION },
                        )))
                    }
                },
                prog if prog == NFS_ACL_PROGRAM
                    || prog == NFS_ID_MAP_PROGRAM
                    || prog == NFS_METADATA_PROGRAM =>
                {
                    trace!("ignoring NFS_ACL/ID_MAP/METADATA packet");
                    Err(ProtocolErrors::RpcAccepted(accept_body::PROG_UNAVAIL))
                }
                NFS_LOCALIO_PROGRAM => {
                    trace!("Ignoring NFS_LOCALIO packet");
                    Err(ProtocolErrors::RpcAccepted(accept_body::PROG_UNAVAIL))
                }
                _ => {
                    warn!("Unknown RPC Program number {} != {}", call.prog, nfs3::PROGRAM);
                    Err(ProtocolErrors::RpcAccepted(accept_body::PROG_UNAVAIL))
                }
            }
        } else {
            error!("Unexpectedly received a Reply instead of a Call");
            Err(ProtocolErrors::RpcAccepted(accept_body::GARBAGE_ARGS))
        }
    }
}
