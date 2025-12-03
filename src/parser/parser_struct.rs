use std::cmp::min;
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
use crate::parser::{Arguments, Error, ProgramVersionMismatch, RPCVersionMismatch, Result};
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
    pub fn new(socket: S, allocator: A) -> Self {
        Self {
            allocator,
            buffer: CountBuffer::new(MAX_MESSAGE_LEN, socket),
            last: false,
            current_frame_size: 0,
        }
    }

    // used only in the beginning of parsing
    async fn fill_buffer(&mut self, min_bytes: usize) -> Result<()> {
        self.buffer.fill_buffer(min_bytes).await.map_err(Error::IO)?;
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
            NFS_PROGRAM => {
                match head.version {
                    NFS_VERSION => {
                        Ok(Box::new(match head.procedure {
                            0 => Arguments::Null,
                            1 => Arguments::GetAttr(get_attr(&mut self.buffer)?),
                            2 => Arguments::SetAttr(set_attr(&mut self.buffer)?),
                            3 => Arguments::LookUp(lookup(&mut self.buffer)?),
                            4 => Arguments::Access(access(&mut self.buffer)?),
                            5 => Arguments::ReadLink(readlink(&mut self.buffer)?),
                            6 => Arguments::Read(read(&mut self.buffer)?),
                            // some other logic with allocator!!!
                            7 => Arguments::Write(
                                adapter_for_write(&mut self.allocator, &mut self.buffer).await?,
                            ),
                            8 => Arguments::Create(create(&mut self.buffer)?),
                            9 => Arguments::MkDir(mkdir(&mut self.buffer)?),
                            10 => Arguments::SymLink(symlink(&mut self.buffer)?),
                            11 => Arguments::MkNod(mknod(&mut self.buffer)?),
                            12 => Arguments::Remove(remove(&mut self.buffer)?),
                            13 => Arguments::RmDir(rmdir(&mut self.buffer)?),
                            14 => Arguments::Rename(rename(&mut self.buffer)?),
                            15 => Arguments::Link(link(&mut self.buffer)?),
                            16 => Arguments::ReadDir(readdir(&mut self.buffer)?),
                            17 => Arguments::ReadDirPlus(readdir_plus(&mut self.buffer)?),
                            18 => Arguments::FsStat(fsstat(&mut self.buffer)?),
                            19 => Arguments::FsInfo(fsinfo(&mut self.buffer)?),
                            20 => Arguments::PathConf(pathconf(&mut self.buffer)?),
                            21 => Arguments::Commit(commit(&mut self.buffer)?),
                            _ => return Err(Error::ProcedureMismatch),
                        }))
                    }
                    _ => Err(Error::ProgramVersionMismatch(ProgramVersionMismatch(
                        NFS_VERSION,
                        NFS_VERSION,
                    ))),
                }
            }

            MOUNT_PROGRAM => {
                if head.version != MOUNT_VERSION {
                    return Err(Error::ProgramVersionMismatch(ProgramVersionMismatch(
                        MOUNT_VERSION,
                        MOUNT_VERSION,
                    )));
                }
                Ok(Box::new(match head.procedure {
                    0 => Arguments::Null,
                    1 => Arguments::Mount(mount(&mut self.buffer)?),
                    2 => Arguments::Dump,
                    3 => Arguments::Unmount(unmount(&mut self.buffer)?),
                    4 => Arguments::UnmountAll,
                    5 => Arguments::Export,
                    _ => return Err(Error::ProcedureMismatch),
                }))
            }
            _ => Err(Error::ProgramMismatch),
        }
    }

    pub async fn parse_message(&mut self) -> Result<Box<Arguments>> {
        // first batch - to parse header
        self.fill_buffer(MIN_MESSAGE_LEN).await?;
        self.read_message_header().await?;
        let rpc_header = self.parse_rpc_header().await;

        // second batch of bytes - what would be done here with write?
        self.fill_buffer(self.current_frame_size - MIN_MESSAGE_LEN).await?;

        let header = rpc_header.inspect_err(|_| {
            self.finalize_parsing();
        })?;

        let auth_status = self.parse_authentication().await?;
        if auth_status != AuthStat::AuthOk {
            return Err(Error::AuthError(auth_status));
        }

        let proc = self.parse_proc(header).await.inspect_err(|_| {
            self.finalize_parsing();
        })?;

        if self.buffer.total_bytes() != self.current_frame_size {
            return Err(Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Unparsed data remaining in frame",
            )));
        }
        // that's done after normal parsing without errors
        self.finalize_parsing();
        Ok(proc)
    }

    // used after successful parsing - with no errors
    fn finalize_parsing(&mut self) {
        self.buffer.clean();
        self.current_frame_size = 0;
        self.last = false;
    }
}

async fn adapter_for_write<S: AsyncRead + Unpin>(
    alloc: &mut impl Allocator,
    buffer: &mut CountBuffer<S>,
) -> Result<WriteArgs> {
    let (object, offset, count, mode, size) = write(buffer)?;
    let mut slice = alloc
        .alloc(NonZeroUsize::new(size).unwrap())
        .await
        .ok_or(Error::IO(io::Error::new(ErrorKind::OutOfMemory, "cannot allocate memory")))?;
    let padding = (ALIGNMENT - size % ALIGNMENT) % ALIGNMENT;
    let from_sync = read_in_slice_sync(buffer, &mut slice, size)?;
    read_in_slice_async(buffer, &mut slice, from_sync, size - from_sync).await?;

    // very ugly!!!

    let mut tmp_buf = [0u8, 0u8, 0u8, 0u8];
    let pad_in_buf = min(buffer.available_read(), padding);
    buffer.read_to_dest(&mut tmp_buf[..pad_in_buf]).map_err(Error::IO)?;
    buffer
        .fill_exact(&mut tmp_buf[..(ALIGNMENT - pad_in_buf) % ALIGNMENT])
        .await
        .map_err(Error::IO)?;

    Ok(WriteArgs { object, offset, count, mode, data: slice })
}
