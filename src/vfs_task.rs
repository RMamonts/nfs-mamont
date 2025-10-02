use std::io;
use std::io::{Cursor, ErrorKind, Read, Write};

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
use crate::xdr::mount::{dirpath, MountProgram};
use crate::xdr::nfs3::dir::{MKDIR3args, READDIR3args, READDIRPLUS3args};
use crate::xdr::nfs3::file::{
    COMMIT3args, CREATE3args, LINK3args, LOOKUP3args, READ3args, WRITE3args,
};
use crate::xdr::nfs3::fs::{FSINFO3args, FSSTAT3args, PATHCONF3args};
use crate::xdr::nfs3::fs_object::{
    ACCESS3args, GETATTR3args, MKNOD3args, READLINK3args, REMOVE3args, RENAME3args, SETATTR3args,
    SYMLINK3args,
};
use crate::xdr::nfs3::NFSProgram;
use crate::xdr::{deserialize, mount, nfs3, rpc, Serialize};

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

        while let Some(mut command) = self.command_receiver.recv().await {
            trace!("Processing command from queue");

            // Clear buffer for reuse
            output_buffer.clear();

            let mut output_cursor = Cursor::new(output_buffer.get_mut_buffer());

            // Call async processor
            let result = match self.handle_rpc(&mut command.data, &mut output_cursor).await {
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
                        let procedure = NFSProgram::from_u32(call.proc).unwrap();
                        let arg = parse_nfs_procedure(input, procedure)?;
                        nfs::v3::handle_nfs(xid, arg, output, &self.context).await
                    }
                    v => {
                        warn!("Unsupported NFS version: {}", v);
                        xdr::rpc::prog_version_range_mismatch_reply_message(
                            xid,
                            nfs3::VERSION,
                            nfs3::VERSION,
                        )
                        .serialize(output)?;
                        Ok(())
                    }
                },
                mount::PROGRAM => {
                    let procedure = MountProgram::from_u32(call.proc).unwrap();
                    let arg = parse_mount_procedure(input, procedure)?;
                    nfs::mount::handle_mount(xid, arg, output, &self.context).await
                }
                prog if prog == NFS_ACL_PROGRAM
                    || prog == NFS_ID_MAP_PROGRAM
                    || prog == NFS_METADATA_PROGRAM =>
                {
                    trace!("ignoring NFS_ACL/ID_MAP/METADATA packet");
                    rpc::prog_unavail_reply_message(xid).serialize(output)?;
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
}

fn parse_nfs_procedure(
    input: &mut impl Read,
    procedure: NFSProgram,
) -> io::Result<Box<nfs3::Args>> {
    Ok(Box::new(match procedure {
        NFSProgram::NFSPROC3_NULL => nfs3::Args::Null,
        NFSProgram::NFSPROC3_GETATTR => nfs3::Args::Getattr(deserialize::<GETATTR3args>(input)?),
        NFSProgram::NFSPROC3_SETATTR => nfs3::Args::Setattr(deserialize::<SETATTR3args>(input)?),
        NFSProgram::NFSPROC3_LOOKUP => nfs3::Args::Lookup(deserialize::<LOOKUP3args>(input)?),
        NFSProgram::NFSPROC3_ACCESS => nfs3::Args::Access(deserialize::<ACCESS3args>(input)?),
        NFSProgram::NFSPROC3_READLINK => nfs3::Args::Readlink(deserialize::<READLINK3args>(input)?),
        NFSProgram::NFSPROC3_READ => nfs3::Args::Read(deserialize::<READ3args>(input)?),
        NFSProgram::NFSPROC3_WRITE => nfs3::Args::Write(deserialize::<WRITE3args>(input)?),
        NFSProgram::NFSPROC3_CREATE => nfs3::Args::Create(deserialize::<CREATE3args>(input)?),
        NFSProgram::NFSPROC3_MKDIR => nfs3::Args::Mkdir(deserialize::<MKDIR3args>(input)?),
        NFSProgram::NFSPROC3_SYMLINK => nfs3::Args::Symlink(deserialize::<SYMLINK3args>(input)?),
        NFSProgram::NFSPROC3_MKNOD => nfs3::Args::Mknod(deserialize::<MKNOD3args>(input)?),
        NFSProgram::NFSPROC3_REMOVE => nfs3::Args::Remove(deserialize::<REMOVE3args>(input)?),
        NFSProgram::NFSPROC3_RMDIR => nfs3::Args::Rmdir(deserialize::<REMOVE3args>(input)?),
        NFSProgram::NFSPROC3_RENAME => nfs3::Args::Rename(deserialize::<RENAME3args>(input)?),
        NFSProgram::NFSPROC3_LINK => nfs3::Args::Link(deserialize::<LINK3args>(input)?),
        NFSProgram::NFSPROC3_READDIR => nfs3::Args::Readdir(deserialize::<READDIR3args>(input)?),
        NFSProgram::NFSPROC3_READDIRPLUS => {
            nfs3::Args::Readdirplus(deserialize::<READDIRPLUS3args>(input)?)
        }
        NFSProgram::NFSPROC3_FSSTAT => nfs3::Args::Fsstat(deserialize::<FSSTAT3args>(input)?),
        NFSProgram::NFSPROC3_FSINFO => nfs3::Args::Fsinfo(deserialize::<FSINFO3args>(input)?),
        NFSProgram::NFSPROC3_PATHCONF => nfs3::Args::Pathconf(deserialize::<PATHCONF3args>(input)?),
        NFSProgram::NFSPROC3_COMMIT => nfs3::Args::Commit(deserialize::<COMMIT3args>(input)?),
        NFSProgram::INVALID => {
            return Err(io::Error::new(ErrorKind::InvalidInput, "invalid proc number"))
        }
    }))
}

fn parse_mount_procedure(
    input: &mut impl Read,
    procedure: MountProgram,
) -> io::Result<Box<mount::Args>> {
    Ok(Box::new(match procedure {
        MountProgram::MOUNTPROC3_NULL => mount::Args::Null,
        MountProgram::MOUNTPROC3_MNT => mount::Args::Mnt(deserialize::<dirpath>(input)?),
        MountProgram::MOUNTPROC3_DUMP => mount::Args::Dump,
        MountProgram::MOUNTPROC3_UMNT => mount::Args::Umnt(deserialize::<dirpath>(input)?),
        MountProgram::MOUNTPROC3_UMNTALL => mount::Args::Umntall,
        MountProgram::MOUNTPROC3_EXPORT => mount::Args::Export,
        MountProgram::INVALID => {
            return Err(io::Error::new(ErrorKind::InvalidInput, "invalid proc number"))
        }
    }))
}
