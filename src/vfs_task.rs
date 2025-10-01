use std::io;
use std::io::{Cursor, Read, Write};
use num_traits::FromPrimitive;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error, trace, warn};

use crate::protocol::nfs;
use crate::protocol::rpc::Context;
use crate::response_buffer::ResponseBuffer;
use crate::rpc_command::RpcCommand;
use crate::tcp::CommandResult;
use crate::utils::error::io_other;
use crate::xdr;
use crate::xdr::{deserialize, mount, nfs3, Serialize};
use crate::xdr::mount::{dirpath, MountProgram};
use crate::xdr::nfs3::{NFSProgram, NFSv3_args};
use crate::xdr::nfs3::dir::{MKDIR3args, READDIR3args, READDIRPLUS3args};
use crate::xdr::nfs3::file::{COMMIT3args, CREATE3args, LINK3args, LOOKUP3args, READ3args, WRITE3args};
use crate::xdr::nfs3::fs::{FSINFO3args, FSSTAT3args, PATHCONF3args};
use crate::xdr::nfs3::fs_object::{ACCESS3args, GETATTR3args, MKNOD3args, READLINK3args, REMOVE3args, RENAME3args, SETATTR3args, SYMLINK3args};
use crate::xdr::rpc::{accept_stat, auth_unix, call_body, rpc_msg};

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

            // Call async processor
            let result = match self.handle_rpc(&mut input_cursor, &mut output_cursor).await {
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
        &mut self,
        input: &mut impl Read,
        output: &mut impl Write,
    ) -> io::Result<bool> {
        let recv = deserialize::<xdr::rpc::rpc_msg>(input)?;
        let xid = recv.xid;
        if let xdr::rpc::rpc_body::CALL(call) = recv.body {
            if let xdr::rpc::auth_flavor::AUTH_SYS = call.cred.flavor {
                let auth = deserialize(&mut Cursor::new(&call.cred.body))?;
                self.context.auth = Some(auth);
            }
            if call.rpcvers != xdr::rpc::PROTOCOL_VERSION {
                warn!("Invalid RPC version {} != 2", call.rpcvers);
                xdr::rpc::rpc_vers_mismatch(xid).serialize(output)?;
                return Ok(true);
            }

            let result = match call.prog {
                nfs3::PROGRAM => match call.vers {
                    nfs3::VERSION => {
                        nfs::v3::handle_nfs(xid, call, input, output, &self.context).await
                    }
                    nfs::v4::VERSION => {
                        nfs::v4::handle_nfs(xid, call, input, output, &self.context).await
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
                mount::PROGRAM => {
                    nfs::mount::handle_mount(xid, call, input, output, &self.context).await
                }
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

            result
        } else {
            error!("Unexpectedly received a Reply instead of a Call");
            io_other("Bad RPC Call format")
        }
    }


fn check_rpc_header(&mut self, recv: rpc_msg, xid: u32) -> Result<call_body, accept_stat> {
    if let xdr::rpc::rpc_body::CALL(call) = recv.body {
        debug!("Receiving call...");
        // authorisation check
        if let xdr::rpc::auth_flavor::AUTH_SYS = call.cred.flavor {
            let auth = deserialize::<auth_unix>(&mut Cursor::new(&call.cred.body))
                .map_err(|_| accept_stat::GARBAGE_ARGS)?;
            self.context.auth = Some(auth);
            // Implement credentials check + move result directly tp WriteTask
        }
        // version of RPC check
        if call.rpcvers != xdr::rpc::PROTOCOL_VERSION {
            warn!("Invalid RPC version {} != 2", call.rpcvers);
            return Err(accept_stat::PROG_MISMATTCH);
            // Go straight to WriteTask?
        }
        Ok(call)
    } else {
        error!("Unexpectedly received a Reply instead of a Call");
        // other type
        Err(accept_stat::GARBAGE_ARGS)
        // Go straight to WriteTask?
    }
}

fn parse_proc_args(
    command: &mut RpcCommand,
    prog: u32,
    proc: u32,
    vers: u32,
    output: &mut impl Write
) -> io::Result<Box<oper_args>, accept_stat> {
    debug!("Parsing arguments");
    match prog {
        nfs3::PROGRAM => match vers {
            nfs3::VERSION => {
                let procedure = NFSProgram::from_u32(proc).unwrap_or(NFSProgram::INVALID);
                debug!("{:?}", procedure);
                parse_nfs_procedure(command, procedure)
            }
            v => {
                warn!("Unsupported NFS version: {}", v);
                Err(accept_stat::PROG_MISMATTCH)
            }
        },
        mount::PROGRAM => {
            let procedure = MountProgram::from_u32(proc).unwrap_or(MountProgram::INVALID);
            debug!("{:?}", procedure);
            Ok(parse_mount_procedure(command, procedure)?)
        }
        prog if prog == NFS_ACL_PROGRAM
            || prog == NFS_ID_MAP_PROGRAM
            || prog == NFS_METADATA_PROGRAM =>
            {
                trace!("ignoring NFS_ACL/ID_MAP/METADATA packet");
                Err(accept_stat::SUCCESS)
            }
        NFS_LOCALIO_PROGRAM => {
            trace!("Ignoring NFS_LOCALIO packet");
            Err(accept_stat::SUCCESS)
        }
        _ => {
            warn!("Unknown RPC Program number {} != {}", prog, nfs3::PROGRAM);
            Err(accept_stat::PROG_UNAVAIL)
        }
    }
}
}

fn parse_nfs_procedure(
    command: &mut RpcCommand,
    procedure: NFSProgram,
) -> io::Result<Box<oper_args>> {
    Ok(Box::new(oper_args::NFS(match procedure {
        NFSProgram::NFSPROC3_NULL => NFSv3_args::NULL,
        NFSProgram::NFSPROC3_GETATTR => {
            NFSv3_args::GETATTR(deserialize::<GETATTR3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_SETATTR => {
            NFSv3_args::SETATTR(deserialize::<SETATTR3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_LOOKUP => {
            NFSv3_args::LOOKUP(deserialize::<LOOKUP3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_ACCESS => {
            NFSv3_args::ACCESS(deserialize::<ACCESS3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_READLINK => {
            NFSv3_args::READLINK(deserialize::<READLINK3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_READ => NFSv3_args::READ(deserialize::<READ3args>(&mut command.data)?),
        NFSProgram::NFSPROC3_WRITE => NFSv3_args::WRITE(deserialize::<WRITE3args>(&mut command.data)?),
        NFSProgram::NFSPROC3_CREATE => {
            NFSv3_args::CREATE(deserialize::<CREATE3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_MKDIR => NFSv3_args::MKDIR(deserialize::<MKDIR3args>(&mut command.data)?),
        NFSProgram::NFSPROC3_SYMLINK => {
            NFSv3_args::SYMLINK(deserialize::<SYMLINK3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_MKNOD => NFSv3_args::MKNOD(deserialize::<MKNOD3args>(&mut command.data)?),
       NFSProgram::NFSPROC3_REMOVE => {
            NFSv3_args::REMOVE(deserialize::<REMOVE3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_RMDIR => NFSv3_args::RMDIR(deserialize::<REMOVE3args>(&mut command.data)?),
        NFSProgram::NFSPROC3_RENAME => {
            NFSv3_args::RENAME(deserialize::<RENAME3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_LINK => NFSv3_args::LINK(deserialize::<LINK3args>(&mut command.data)?),
        NFSProgram::NFSPROC3_READDIR => {
            NFSv3_args::READDIR(deserialize::<READDIR3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_READDIRPLUS => {
            NFSv3_args::READDIRPLUS(deserialize::<READDIRPLUS3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_FSSTAT => {
            NFSv3_args::FSSTAT(deserialize::<FSSTAT3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_FSINFO => NFSv3_args::FSINFO(deserialize::<FSINFO3args>(&mut command.data).unwrap()),
        NFSProgram::NFSPROC3_PATHCONF => {
            NFSv3_args::PATHCONF(deserialize::<PATHCONF3args>(&mut command.data)?)
        }
        NFSProgram::NFSPROC3_COMMIT => {
            NFSv3_args::COMMIT(deserialize::<COMMIT3args>(&mut command.data)?)
        }
        NFSProgram::INVALID => return Err(NFSv3_args::INVALID),
    })))
}




fn parse_mount_procedure(
    command: &mut RpcCommand,
    procedure: MountProgram,
) -> io::Result<Box<oper_args>> {
    trace!("Parsing Mount");
    Ok(Box::new(oper_args::MOUNT(match procedure {
        MountProgram::MOUNTPROC3_NULL => Mount_args::NULL,
        MountProgram::MOUNTPROC3_MNT => Mount_args::MNT(deserialize::<dirpath>(&mut command.data)?),
        MountProgram::MOUNTPROC3_DUMP => Mount_args::DUMP,
        MountProgram::MOUNTPROC3_UMNT => Mount_args::UMNT(deserialize::<dirpath>(&mut command.data)?),
        MountProgram::MOUNTPROC3_UMNTALL => Mount_args::UMNTALL,
        MountProgram::MOUNTPROC3_EXPORT => Mount_args::EXPORT,
        MountProgram::INVALID => return Err(crate::protocol::xdr::mount::Mount_args),
    })))
}

#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
pub enum oper_args {
    NFS(NFSv3_args),
    MOUNT(Mount_args),
}

