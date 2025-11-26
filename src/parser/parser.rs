use std::cmp::min;
use std::io;
use std::io::{ErrorKind, Read};

use log::error;
use tokio::io::AsyncReadExt;
use tokio::net::tcp::OwnedReadHalf;

use crate::mount::{MOUNT_PROGRAM, MOUNT_VERSION};
use crate::nfsv3::{NFS_PROGRAM, NFS_VERSION};
use crate::parser::mount::{mount, unmount, MountArgs, UnmountArgs};
use crate::parser::nfsv3::procedures::{
    access, commit, create, fsinfo, fsstat, get_attr, link, lookup, mkdir, mknod, pathconf, read,
    readdir, readdir_plus, readlink, remove, rename, rmdir, set_attr, symlink, write, AccessArgs,
    CommitArgs, CreateArgs, FsInfoArgs, FsStatArgs, GetAttrArgs, LinkArgs, LookUpArgs, MkDirArgs,
    MkNodArgs, PathConfArgs, ReadArgs, ReadDirArgs, ReadDirPlusArgs, ReadLinkArgs, RemoveArgs,
    RenameArgs, RmDirArgs, SetAttrArgs, SymLinkArgs, WriteArgs,
};
use crate::parser::primitive::u32;
use crate::parser::rpc::AuthStat;
use crate::parser::{Error, Result};
use crate::rpc::{rpc_body, RPC_VERSION};

// recalculate
const MAX_MESSAGE_LEN: usize = 1500;
const MIN_MESSAGE_LEN: usize = 24;

struct RpcMessage {
    program: u32,
    procedure: u32,
    version: u32,
}

struct CustomCursor {
    buffer: Vec<u8>,
    position: usize,
    read_bytes: usize,
}

impl CustomCursor {
    fn new(size: usize) -> Self {
        Self { buffer: vec![0u8; size], position: 0, read_bytes: 0 }
    }
    fn writer_slice(&mut self) -> &mut [u8] {
        &mut self.buffer[self.read_bytes..]
    }

    fn extend_read_bytes(&mut self, n: usize) {
        self.read_bytes += n;
    }

    fn available_to_read(&mut self) -> usize {
        self.read_bytes - self.position
    }

    fn position(&self) -> usize {
        self.position
    }
}

impl Read for CustomCursor {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = min(buf.len(), self.read_bytes) - self.position;
        if len == 0 {
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "No more data available"));
        }
        buf[..len].copy_from_slice(&self.buffer[self.position..self.read_bytes]);
        self.position += len;
        Ok(len)
    }
}

// need my own cursor!!!
struct Parser {
    // actual allocator
    allocator: (),
    buffer: CustomCursor,
    socket: OwnedReadHalf,
    last: bool,
    frame_size: usize,
}

impl Parser {
    // what with allocator?
    #[allow(dead_code)]
    pub fn new(socket: OwnedReadHalf) -> Self {
        Self {
            allocator: (),
            buffer: CustomCursor::new(MAX_MESSAGE_LEN),
            socket,
            last: false,
            frame_size: 0,
        }
    }

    #[allow(dead_code)]
    async fn read_with_check<T>(
        &mut self,
        caller: impl Fn(&mut dyn Read) -> Result<T>,
    ) -> Result<T> {
        // there is no need to check if we reach end of buffer while appending data to buffer since we have buffer, that would
        // definitely be enough to read what we are planning
        match caller(&mut self.buffer) {
            Err(Error::IO(err)) if err.kind() == ErrorKind::UnexpectedEof => {
                // called whenever we need to read more data
                let bytes_read = self.socket.read(self.buffer.writer_slice()).await;
                match bytes_read {
                    Ok(0) => {
                        // closing connection
                        // or
                        // means that we exceed size - > need to use allocator (that only possible with write!!!)
                        Err(Error::IO(err))
                    }
                    Ok(n) => {
                        self.buffer.extend_read_bytes(n);
                        Box::pin(self.read_with_check(caller)).await
                    }
                    Err(e) => Err(Error::IO(e)),
                }
            }
            Err(err) => Err(err),
            Ok(value) => Ok(value),
        }
    }

    #[allow(dead_code)]
    async fn initial_read(&mut self) -> Result<()> {
        // called whenever we need to read more data
        while self.buffer.available_to_read() < MIN_MESSAGE_LEN {
            let size = self.socket.read(self.buffer.writer_slice()).await.map_err(Error::IO)?;
            self.buffer.extend_read_bytes(size);
        }

        // new cursor
        // parsing header
        let head = u32(&mut self.buffer)?;
        self.last = head & 0x8000_0000 == 0x8000_0000;
        self.frame_size = (head & 0x7FFF_FFFF) as usize;

        Ok(())
    }

    // does not look good
    #[allow(dead_code)]
    pub async fn parse_new_message(&mut self) -> Result<Box<Arguments>> {
        // do some errors checking
        if let Err(error) = self.initial_read().await {
            error!("{error:?} occur while reading from socket");
            return Err(error);
        }

        // do some errors checking
        let msg = match self.parse_header().await {
            Ok(msg) => msg,
            Err(error) => {
                // make some matching
                return Err(error);
            }
        };
        let procedure = self.parse_proc(msg).await?;

        if self.buffer.position() != self.frame_size {
            return Err(Error::IO(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Parsed data don't match frame size",
            )));
        }

        Ok(procedure)
    }

    #[allow(dead_code)]
    async fn parse_header(&mut self) -> Result<RpcMessage> {
        // parse rpc header with all needed checks
        let msg_type = u32(&mut self.buffer)?;
        if msg_type == rpc_body::REPLY as u32 {
            error!("Receive RPC reply message");
            // do we need to do it here? Maybe in read_new_message?
            return Err(Error::MessageTypeMismatch);
        }

        let prc_vers = u32(&mut self.buffer)?;

        if prc_vers != RPC_VERSION {
            error!("RPC version mismatch");
            return Err(Error::RpcVersionMismatch);
        }

        let program = u32(&mut self.buffer)?;
        let version = u32(&mut self.buffer)?;
        let procedure = u32(&mut self.buffer)?;

        let auth = self.authentication().await?;
        if auth != AuthStat::AuthOk {
            error!("Authentication failed: {auth:?}");
            return Err(Error::AuthError(auth));
        }

        Ok(RpcMessage { program, procedure, version })
    }

    async fn parse_proc(&mut self, head: RpcMessage) -> Result<Box<Arguments>> {
        match head.program {
            NFS_PROGRAM => {
                match head.version {
                    NFS_VERSION => {
                        //proc analysis
                        Ok(Box::new(match head.procedure {
                            0 => Arguments::Null,
                            1 => Arguments::GetAttr(self.read_with_check(get_attr).await?),
                            2 => Arguments::SetAttr(self.read_with_check(set_attr).await?),
                            3 => Arguments::LookUp(self.read_with_check(lookup).await?),
                            4 => Arguments::Access(self.read_with_check(access).await?),
                            5 => Arguments::ReadLink(self.read_with_check(readlink).await?),
                            6 => Arguments::Read(self.read_with_check(read).await?),
                            // some other logic with allocator!!!
                            7 => Arguments::Write(self.read_with_check(write).await?),
                            8 => Arguments::Create(self.read_with_check(create).await?),
                            9 => Arguments::MkDir(self.read_with_check(mkdir).await?),
                            10 => Arguments::SymLink(self.read_with_check(symlink).await?),
                            11 => Arguments::MkNod(self.read_with_check(mknod).await?),
                            12 => Arguments::Remove(self.read_with_check(remove).await?),
                            13 => Arguments::RmDir(self.read_with_check(rmdir).await?),
                            14 => Arguments::Rename(self.read_with_check(rename).await?),
                            15 => Arguments::Link(self.read_with_check(link).await?),
                            16 => Arguments::ReadDir(self.read_with_check(readdir).await?),
                            17 => Arguments::ReadDirPlus(self.read_with_check(readdir_plus).await?),
                            18 => Arguments::FsStat(self.read_with_check(fsstat).await?),
                            19 => Arguments::FsInfo(self.read_with_check(fsinfo).await?),
                            20 => Arguments::PathConf(self.read_with_check(pathconf).await?),
                            21 => Arguments::Commit(self.read_with_check(commit).await?),
                            _ => return Err(Error::ProcedureMismatch),
                        }))
                    }
                    _ => {
                        // version mismatch
                        Err(Error::ProgramVersionMismatch)
                    }
                }
            }

            MOUNT_PROGRAM => {
                if head.version != MOUNT_VERSION {
                    return Err(Error::ProgramVersionMismatch);
                }
                // proc analysis
                Ok(Box::new(match head.procedure {
                    0 => Arguments::Null,
                    1 => Arguments::Mount(self.read_with_check(mount).await?),
                    2 => Arguments::Dump,
                    3 => Arguments::Unmount(self.read_with_check(unmount).await?),
                    4 => Arguments::UnmountAll,
                    5 => Arguments::Export,
                    _ => return Err(Error::ProcedureMismatch),
                }))
            }
            _ => {
                // here is prog mismatch
                Err(Error::ProgramMismatch)
            }
        }
    }

    async fn authentication(&mut self) -> Result<AuthStat> {
        todo!()
    }
}

#[allow(dead_code)]
pub enum Arguments {
    // NSGv3
    Null,
    GetAttr(GetAttrArgs),
    SetAttr(SetAttrArgs),
    LookUp(LookUpArgs),
    Access(AccessArgs),
    ReadLink(ReadLinkArgs),
    Read(ReadArgs),
    Write(WriteArgs),
    Create(CreateArgs),
    MkDir(MkDirArgs),
    SymLink(SymLinkArgs),
    MkNod(MkNodArgs),
    Remove(RemoveArgs),
    RmDir(RmDirArgs),
    Rename(RenameArgs),
    Link(LinkArgs),
    ReadDir(ReadDirArgs),
    ReadDirPlus(ReadDirPlusArgs),
    FsStat(FsStatArgs),
    FsInfo(FsInfoArgs),
    PathConf(PathConfArgs),
    Commit(CommitArgs),
    // MOUNT
    Mount(MountArgs),
    Unmount(UnmountArgs),
    Export,
    Dump,
    UnmountAll,
    ExportAll,
}
