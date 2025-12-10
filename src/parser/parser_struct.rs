use std::io::{self, ErrorKind};
use std::num::NonZeroUsize;

use tokio::io::AsyncRead;

use crate::allocator::Allocator;
use crate::mount::{MOUNT_PROGRAM, MOUNT_VERSION};
use crate::nfsv3::{NFS_PROGRAM, NFS_VERSION};
use crate::parser::mount::{mount, unmount};
use crate::parser::nfsv3::procedures::{
    access, commit, create, fsinfo, fsstat, get_attr, link, lookup, mkdir, mknod, pathconf, read,
    read_in_slice_async, read_in_slice_sync, readdir, readdir_plus, readlink, remove, rename,
    rmdir, set_attr, symlink, write, WriteArgs,
};
use crate::parser::primitive::{u32, ALIGNMENT};
use crate::parser::read_buffer::CountBuffer;
use crate::parser::rpc::{auth, AuthFlavor, AuthStat, RpcMessage};
use crate::parser::{
    proc_nested_errors, Arguments, Error, ProgramVersionMismatch, RPCVersionMismatch, Result,
};
use crate::rpc::{rpc_message_type, RPC_VERSION};

#[allow(dead_code)]
const MAX_MESSAGE_LEN: usize = 2500;
#[allow(dead_code)]
const MIN_MESSAGE_LEN: usize = 36;

pub struct RpcParser<A: Allocator, S: AsyncRead + Unpin> {
    allocator: A,
    buffer: CountBuffer<S>,
    last: bool,
    current_frame_size: usize,
}

#[allow(dead_code)]
impl<A: Allocator, S: AsyncRead + Unpin> RpcParser<A, S> {
    pub fn new(socket: S, allocator: A, size: usize) -> Self {
        Self {
            allocator,
            buffer: CountBuffer::new(size, socket),
            last: false,
            current_frame_size: 0,
        }
    }

    // used only in the beginning of parsing
    async fn fill_buffer(&mut self, min_bytes: usize) -> Result<()> {
        while self.buffer.available_read() < min_bytes {
            match self.buffer.fill_internal().await {
                Ok(_) => continue,
                Err(err) => return Err(Error::IO(err)),
            }
        }
        Ok(())
    }

    // here we are positively sure, that we can read without retry function
    async fn read_message_header(&mut self) -> Result<()> {
        let header = u32(&mut self.buffer)?;
        self.last = header & 0x8000_0000 != 0;
        self.current_frame_size = (header & 0x7FFF_FFFF) as usize;

        // this is temporal check, apparently this will go to separate object Validator
        if !self.last {
            return Err(Error::IO(io::Error::new(
                ErrorKind::Unsupported,
                "Fragmented messages not supported",
            )));
        }
        let _xid = u32(&mut self.buffer)?;
        Ok(())
    }

    // here we are positively sure, that we can read without retry function,
    // because there is Minimum size
    async fn parse_rpc_header(&mut self) -> Result<RpcMessage> {
        let msg_type = u32(&mut self.buffer)?;
        if msg_type == rpc_message_type::REPLY as u32 {
            return Err(Error::MessageTypeMismatch);
        }

        let rpc_version = u32(&mut self.buffer)?;
        if rpc_version != RPC_VERSION {
            return Err(Error::RpcVersionMismatch(RPCVersionMismatch(RPC_VERSION, RPC_VERSION)));
        }

        let program = u32(&mut self.buffer)?;
        let version = u32(&mut self.buffer)?;
        let procedure = u32(&mut self.buffer)?;

        let auth_status = self.parse_authentication().await?;
        if auth_status != AuthStat::AuthOk {
            return Err(Error::AuthError(auth_status));
        }

        Ok(RpcMessage { program, procedure, version })
    }

    async fn parse_authentication(&mut self) -> Result<AuthStat> {
        match auth(&mut self.buffer)?.flavor {
            AuthFlavor::AuthNone => Ok(AuthStat::AuthOk),
            _ => {
                unimplemented!()
            }
        }
    }

    async fn parse_proc(&mut self, head: RpcMessage) -> Result<Box<Arguments>> {
        match head.program {
            NFS_PROGRAM => match head.version {
                NFS_VERSION => Ok(Box::new(match head.procedure {
                    0 => Arguments::Null,
                    1 => Arguments::GetAttr(self.buffer.parse_with_retry(get_attr).await?),
                    2 => Arguments::SetAttr(self.buffer.parse_with_retry(set_attr).await?),
                    3 => Arguments::LookUp(self.buffer.parse_with_retry(lookup).await?),
                    4 => Arguments::Access(self.buffer.parse_with_retry(access).await?),
                    5 => Arguments::ReadLink(self.buffer.parse_with_retry(readlink).await?),
                    6 => Arguments::Read(self.buffer.parse_with_retry(read).await?),

                    7 => Arguments::Write(
                        adapter_for_write(&mut self.allocator, &mut self.buffer).await?,
                    ),

                    8 => Arguments::Create(self.buffer.parse_with_retry(create).await?),
                    9 => Arguments::MkDir(self.buffer.parse_with_retry(mkdir).await?),
                    10 => Arguments::SymLink(self.buffer.parse_with_retry(symlink).await?),
                    11 => Arguments::MkNod(self.buffer.parse_with_retry(mknod).await?),
                    12 => Arguments::Remove(self.buffer.parse_with_retry(remove).await?),
                    13 => Arguments::RmDir(self.buffer.parse_with_retry(rmdir).await?),
                    14 => Arguments::Rename(self.buffer.parse_with_retry(rename).await?),
                    15 => Arguments::Link(self.buffer.parse_with_retry(link).await?),
                    16 => Arguments::ReadDir(self.buffer.parse_with_retry(readdir).await?),
                    17 => Arguments::ReadDirPlus(self.buffer.parse_with_retry(readdir_plus).await?),
                    18 => Arguments::FsStat(self.buffer.parse_with_retry(fsstat).await?),
                    19 => Arguments::FsInfo(self.buffer.parse_with_retry(fsinfo).await?),
                    20 => Arguments::PathConf(self.buffer.parse_with_retry(pathconf).await?),
                    21 => Arguments::Commit(self.buffer.parse_with_retry(commit).await?),
                    _ => return Err(Error::ProcedureMismatch),
                })),
                _ => Err(Error::ProgramVersionMismatch(ProgramVersionMismatch(
                    NFS_VERSION,
                    NFS_VERSION,
                ))),
            },

            MOUNT_PROGRAM => {
                if head.version != MOUNT_VERSION {
                    return Err(Error::ProgramVersionMismatch(ProgramVersionMismatch(
                        MOUNT_VERSION,
                        MOUNT_VERSION,
                    )));
                }
                Ok(Box::new(match head.procedure {
                    0 => Arguments::Null,
                    1 => Arguments::Mount(self.buffer.parse_with_retry(mount).await?),
                    2 => Arguments::Dump,
                    3 => Arguments::Unmount(self.buffer.parse_with_retry(unmount).await?),
                    4 => Arguments::UnmountAll,
                    5 => Arguments::Export,
                    _ => return Err(Error::ProcedureMismatch),
                }))
            }
            _ => Err(Error::ProgramMismatch),
        }
    }

    pub async fn parse_message(&mut self) -> Result<Box<Arguments>> {
        self.fill_buffer(MIN_MESSAGE_LEN).await?;
        self.read_message_header().await?;
        let rpc_header = match self.parse_rpc_header().await {
            Ok(arg) => arg,
            Err(err) => return Err(self.match_errors(err).await),
        };
        let proc = match self.parse_proc(rpc_header).await {
            Ok(arg) => arg,
            Err(err) => return Err(self.match_errors(err).await),
        };

        // that's done after normal parsing without errors
        self.finalize_parsing()?;
        Ok(proc)
    }

    // used after successful parsing - with no errors
    fn finalize_parsing(&mut self) -> Result<()> {
        if self.buffer.total_bytes() != self.current_frame_size {
            return Err(Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Unparsed data remaining in frame",
            )));
        }

        self.buffer.clean();
        self.current_frame_size = 0;
        self.last = false;
        Ok(())
    }

    async fn match_errors(&mut self, error: Error) -> Error {
        if let Error::RpcVersionMismatch(_)
        | Error::ProgramMismatch
        | Error::ProcedureMismatch
        | Error::AuthError(_)
        | Error::ProgramVersionMismatch(_) = &error
        {
            proc_nested_errors(error, self.discard_current_message()).await
        } else {
            error
        }
    }

    // used after non-fatal errors - to clean remaining data from socket
    // these would work with set of errors we are going to use if for
    async fn discard_current_message(&mut self) -> Result<()> {
        let remaining = self.current_frame_size - self.buffer.total_bytes();
        self.buffer.discard_bytes(remaining).await.map_err(Error::IO)?;
        self.finalize_parsing()?;
        Ok(())
    }
}

async fn adapter_for_write<S: AsyncRead + Unpin>(
    alloc: &mut impl Allocator,
    buffer: &mut CountBuffer<S>,
) -> Result<WriteArgs> {
    let (object, offset, count, mode, size) = buffer.parse_with_retry(write).await?;
    let mut slice = alloc
        .allocate(NonZeroUsize::new(size).unwrap())
        .await
        .ok_or(Error::IO(io::Error::new(ErrorKind::OutOfMemory, "cannot allocate memory")))?;
    let padding = (ALIGNMENT - size % ALIGNMENT) % ALIGNMENT;
    let from_sync = read_in_slice_sync(buffer, &mut slice, size)?;
    read_in_slice_async(buffer, &mut slice, from_sync, size - from_sync).await?;
    buffer.discard_bytes(padding).await.map_err(Error::IO)?;
    Ok(WriteArgs { object, offset, count, mode, data: slice })
}
