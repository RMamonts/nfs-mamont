use std::cmp::min;
use std::io::{self, ErrorKind, Read, Write};

use tokio::io::{AsyncRead, AsyncReadExt};
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

const MAX_MESSAGE_LEN: usize = 1500;
const MIN_MESSAGE_LEN: usize = 24;

#[derive(Debug)]
struct RpcMessage {
    program: u32,
    procedure: u32,
    version: u32,
}

struct ReadBuffer {
    data: Vec<u8>,
    read_pos: usize,
    write_pos: usize,
}

#[allow(dead_code)]
impl ReadBuffer {
    fn new(capacity: usize) -> Self {
        Self { data: vec![0u8; capacity], read_pos: 0, write_pos: 0 }
    }

    fn bytes_read(&self) -> usize {
        self.read_pos
    }
    fn available_read(&self) -> usize {
        self.write_pos - self.read_pos
    }

    fn available_write(&self) -> usize {
        self.data.len() - self.write_pos
    }

    fn read_slice(&self) -> &[u8] {
        &self.data[self.read_pos..self.write_pos]
    }

    fn write_slice(&mut self) -> &mut [u8] {
        &mut self.data[self.write_pos..]
    }

    fn consume(&mut self, n: usize) {
        self.read_pos += n;
    }

    fn extend(&mut self, n: usize) {
        self.write_pos += n;
    }

    fn compact(&mut self) {
        if self.read_pos > 0 {
            self.data.copy_within(self.read_pos..self.write_pos, 0);
            self.write_pos -= self.read_pos;
            self.read_pos = 0;
        }
    }

    fn clear(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
    }

    fn len(&self) -> usize {
        self.data.len()
    }
}

impl Read for ReadBuffer {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = min(buf.len(), self.available_read());
        buf[..len].copy_from_slice(&self.data[self.read_pos..self.read_pos + len]);
        self.consume(len);
        Ok(len)
    }
}

impl Write for ReadBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = min(buf.len(), self.available_write());
        self.write_slice()[..len].copy_from_slice(&buf[..len]);
        self.extend(len);
        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct RpcParser {
    buffer: ReadBuffer,
    socket: OwnedReadHalf,
    last: bool,
    current_frame_size: usize,
}

#[allow(dead_code)]
impl RpcParser {
    pub fn new(socket: OwnedReadHalf, size: usize) -> Self {
        Self { buffer: ReadBuffer::new(size), socket, last: false, current_frame_size: 0 }
    }

    // used only in the beginning of parsing
    async fn fill_buffer(&mut self, min_bytes: usize) -> Result<()> {
        while self.buffer.available_read() < min_bytes {
            if self.buffer.available_write() == 0 {
                return Err(Error::IO(io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "Buffer exhausted",
                )));
            }
        }

        let bytes_read = self.socket.read(self.buffer.write_slice()).await.map_err(Error::IO)?;

        if bytes_read == 0 {
            return Err(Error::IO(io::Error::new(ErrorKind::UnexpectedEof, "Connection closed")));
        }

        self.buffer.extend(bytes_read);

        Ok(())
    }

    async fn read_message_header(&mut self) -> Result<()> {
        self.fill_buffer(MIN_MESSAGE_LEN).await?;

        let header = u32(&mut self.buffer)?;
        self.last = header & 0x8000_0000 != 0;
        self.current_frame_size = (header & 0x7FFF_FFFF) as usize;

        // find out about these two checks

        if self.current_frame_size > MAX_MESSAGE_LEN {
            return Err(Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Frame size exceeds maximum",
            )));
        }

        if !self.last {
            return Err(Error::IO(io::Error::new(
                ErrorKind::Unsupported,
                "Fragmented messages not supported",
            )));
        }

        Ok(())
    }

    async fn parse_rpc_header(&mut self) -> Result<RpcMessage> {
        let msg_type = u32(&mut self.buffer)?;
        if msg_type == rpc_body::REPLY as u32 {
            return Err(Error::MessageTypeMismatch);
        }

        let rpc_version = u32(&mut self.buffer)?;
        if rpc_version != RPC_VERSION {
            return Err(Error::RpcVersionMismatch);
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

    async fn parse_proc(&mut self, head: RpcMessage) -> Result<Box<Arguments>> {
        match head.program {
            NFS_PROGRAM => {
                match head.version {
                    NFS_VERSION => {
                        Ok(Box::new(match head.procedure {
                            0 => Arguments::Null,
                            1 => Arguments::GetAttr(
                                parse_with_retry(&mut self.buffer, &mut self.socket, get_attr)
                                    .await?,
                            ),
                            2 => Arguments::SetAttr(
                                parse_with_retry(&mut self.buffer, &mut self.socket, set_attr)
                                    .await?,
                            ),
                            3 => Arguments::LookUp(
                                parse_with_retry(&mut self.buffer, &mut self.socket, lookup)
                                    .await?,
                            ),
                            4 => Arguments::Access(
                                parse_with_retry(&mut self.buffer, &mut self.socket, access)
                                    .await?,
                            ),
                            5 => Arguments::ReadLink(
                                parse_with_retry(&mut self.buffer, &mut self.socket, readlink)
                                    .await?,
                            ),
                            6 => Arguments::Read(
                                parse_with_retry(&mut self.buffer, &mut self.socket, read).await?,
                            ),
                            // some other logic with allocator!!!
                            7 => Arguments::Write(
                                parse_with_retry(&mut self.buffer, &mut self.socket, write).await?,
                            ),

                            8 => Arguments::Create(
                                parse_with_retry(&mut self.buffer, &mut self.socket, create)
                                    .await?,
                            ),
                            9 => Arguments::MkDir(
                                parse_with_retry(&mut self.buffer, &mut self.socket, mkdir).await?,
                            ),
                            10 => Arguments::SymLink(
                                parse_with_retry(&mut self.buffer, &mut self.socket, symlink)
                                    .await?,
                            ),
                            11 => Arguments::MkNod(
                                parse_with_retry(&mut self.buffer, &mut self.socket, mknod).await?,
                            ),
                            12 => Arguments::Remove(
                                parse_with_retry(&mut self.buffer, &mut self.socket, remove)
                                    .await?,
                            ),
                            13 => Arguments::RmDir(
                                parse_with_retry(&mut self.buffer, &mut self.socket, rmdir).await?,
                            ),
                            14 => Arguments::Rename(
                                parse_with_retry(&mut self.buffer, &mut self.socket, rename)
                                    .await?,
                            ),
                            15 => Arguments::Link(
                                parse_with_retry(&mut self.buffer, &mut self.socket, link).await?,
                            ),
                            16 => Arguments::ReadDir(
                                parse_with_retry(&mut self.buffer, &mut self.socket, readdir)
                                    .await?,
                            ),
                            17 => Arguments::ReadDirPlus(
                                parse_with_retry(&mut self.buffer, &mut self.socket, readdir_plus)
                                    .await?,
                            ),
                            18 => Arguments::FsStat(
                                parse_with_retry(&mut self.buffer, &mut self.socket, fsstat)
                                    .await?,
                            ),
                            19 => Arguments::FsInfo(
                                parse_with_retry(&mut self.buffer, &mut self.socket, fsinfo)
                                    .await?,
                            ),
                            20 => Arguments::PathConf(
                                parse_with_retry(&mut self.buffer, &mut self.socket, pathconf)
                                    .await?,
                            ),
                            21 => Arguments::Commit(
                                parse_with_retry(&mut self.buffer, &mut self.socket, commit)
                                    .await?,
                            ),
                            _ => return Err(Error::ProcedureMismatch),
                        }))
                    }
                    _ => Err(Error::ProgramVersionMismatch),
                }
            }

            MOUNT_PROGRAM => {
                if head.version != MOUNT_VERSION {
                    return Err(Error::ProgramVersionMismatch);
                }
                Ok(Box::new(match head.procedure {
                    0 => Arguments::Null,
                    1 => Arguments::Mount(
                        parse_with_retry(&mut self.buffer, &mut self.socket, mount).await?,
                    ),
                    2 => Arguments::Dump,
                    3 => Arguments::Unmount(
                        parse_with_retry(&mut self.buffer, &mut self.socket, unmount).await?,
                    ),
                    4 => Arguments::UnmountAll,
                    5 => Arguments::Export,
                    _ => return Err(Error::ProcedureMismatch),
                }))
            }
            _ => Err(Error::ProgramMismatch),
        }
    }

    // TODO: add some error handling
    pub async fn parse_message(&mut self) -> Result<Box<Arguments>> {
        self.read_message_header().await?;
        let rpc_header = self.parse_rpc_header().await?;
        let procedure = self.parse_proc(rpc_header).await?;
        self.finalize_parsing()?;
        Ok(procedure)
    }

    // used after successful parsing - with no errors
    fn finalize_parsing(&mut self) -> Result<()> {
        if self.buffer.bytes_read() != self.current_frame_size {
            return Err(Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Unparsed data remaining in frame",
            )));
        }

        self.buffer.compact();
        self.current_frame_size = 0;
        self.last = false;
        Ok(())
    }

    // used after non-fatal errors - to clean remaining data from socket
    // !!! implement some logic for write - we would need allocator
    pub async fn discard_current_message(&mut self) -> Result<()> {
        let remaining = self.current_frame_size - self.buffer.bytes_read();

        let mut total_discarded = 0;
        while total_discarded < remaining {
            self.buffer.clear();
            let read = self.socket.read(self.buffer.write_slice()).await.map_err(Error::IO)?;
            if read == 0 {
                break;
            }
            total_discarded += read;
        }
        self.buffer.clear();
        self.current_frame_size = 0;
        self.last = false;
        Ok(())
    }

    async fn parse_authentication(&mut self) -> Result<AuthStat> {
        // TODO
        Ok(AuthStat::AuthOk)
    }
}

#[allow(dead_code)]
async fn parse_with_retry<T>(
    buffer: &mut ReadBuffer,
    socket: &mut (impl AsyncRead + Unpin),
    caller: impl Fn(&mut dyn Read) -> Result<T>,
) -> Result<T> {
    // there is no need to check if we reach end of buffer while appending data to buffer since we have buffer, that would
    // definitely be enough to read what we are planning
    match caller(buffer) {
        Err(Error::IO(err)) if err.kind() == ErrorKind::UnexpectedEof => {
            // called whenever we need to read more data
            let bytes_read = socket.read(buffer.write_slice()).await;
            match bytes_read {
                Ok(0) => {
                    // closing connection
                    // or
                    // means that we exceed size - > need to use allocator (that only possible with write!!!)
                    Err(Error::IO(err))
                }
                Ok(n) => {
                    buffer.extend(n);
                    Box::pin(parse_with_retry(buffer, socket, caller)).await
                }
                Err(e) => Err(Error::IO(e)),
            }
        }
        result => result,
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Arguments {
    // NFSv3
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
}
