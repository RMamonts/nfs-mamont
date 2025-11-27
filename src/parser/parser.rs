use crate::allocator::Allocator;
use crate::mount::{MOUNT_PROGRAM, MOUNT_VERSION};
use crate::nfsv3::{NFS_PROGRAM, NFS_VERSION};
use crate::parser::mount::{mount, unmount, MountArgs, UnmountArgs};
use crate::parser::nfsv3::procedures::{
    access, commit, create, fsinfo, fsstat, get_attr, link, lookup, mkdir, mknod, pathconf, read,
    read_in_slice_async, read_in_slice_sync, readdir, readdir_plus, readlink, remove, rename,
    rmdir, set_attr, symlink, write, AccessArgs, CommitArgs, CreateArgs, FsInfoArgs, FsStatArgs,
    GetAttrArgs, LinkArgs, LookUpArgs, MkDirArgs, MkNodArgs, PathConfArgs, ReadArgs, ReadDirArgs,
    ReadDirPlusArgs, ReadLinkArgs, RemoveArgs, RenameArgs, RmDirArgs, SetAttrArgs, SymLinkArgs,
    WriteArgs,
};
use crate::parser::primitive::{u32, ALIGNMENT};
use crate::parser::rpc::{auth, AuthFlavor, AuthStat};
use crate::parser::{process_suberror, Error, ProgramVersionMismatch, RPCVersionMismatch, Result};
use crate::rpc::{rpc_body, RPC_VERSION};
use std::cmp::min;
use std::io::{self, ErrorKind, Read, Write};
use std::num::NonZeroUsize;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::tcp::OwnedReadHalf;

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

    // maybe need some check before doing smt like this???
    fn construct_slice(&mut self, size: usize) -> &mut [u8] {
        &mut self.data[self.read_pos..self.read_pos + size]
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

pub struct RpcParser<A: Allocator> {
    allocator: A,
    buffer: ReadBuffer,
    socket: OwnedReadHalf,
    last: bool,
    current_frame_size: usize,
}

#[allow(dead_code)]
impl<A: Allocator> RpcParser<A> {
    pub fn new(socket: OwnedReadHalf, size: usize, allocator: A) -> Self {
        Self {
            allocator,
            buffer: ReadBuffer::new(size),
            socket,
            last: false,
            current_frame_size: 0,
        }
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
                                adapter_for_write(
                                    &mut self.allocator,
                                    &mut self.buffer,
                                    &mut self.socket,
                                    self.current_frame_size,
                                )
                                .await?,
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

    async fn match_errors(&mut self, error: Error) -> Error {
        match error {
            //these one to send to write directly to writetask and continue parsing
            //use discard_current_message

            //these one are fatal, but some still need replies (parse_error and server_error)
            //probably simply drop everything after sending sata to other tasks
            Error::RpcVersionMismatch(arg) => {
                process_suberror(Error::RpcVersionMismatch(arg), self.discard_current_message())
                    .await
            }
            Error::ProgramMismatch => {
                process_suberror(Error::ProgramMismatch, self.discard_current_message()).await
            }
            Error::ProcedureMismatch => {
                process_suberror(Error::ProcedureMismatch, self.discard_current_message()).await
            }
            Error::AuthError(e) => {
                process_suberror(Error::AuthError(e), self.discard_current_message()).await
            }
            Error::ProgramVersionMismatch(arg) => {
                process_suberror(Error::ProgramVersionMismatch(arg), self.discard_current_message())
                    .await
            }
            // these are fatal errors, that would result in dropping current task - no need doing cleaning
            er => er,
        }
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
        match auth(&mut self.buffer)?.flavor {
            AuthFlavor::AuthNone => Ok(AuthStat::AuthOk),
            _ => {
                unimplemented!()
            }
        }
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
            match socket.read(buffer.write_slice()).await {
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

// TODO: review all size checks i do (probably change usize with isize)
async fn adapter_for_write(
    alloc: &mut impl Allocator,
    buffer: &mut ReadBuffer,
    socket: &mut (impl Unpin + AsyncRead),
    mut bytes_left: usize,
) -> Result<WriteArgs> {
    const SKIP_SIZE: usize = 28;
    let padding = (ALIGNMENT - (bytes_left - SKIP_SIZE) % ALIGNMENT) % ALIGNMENT;
    let opaque_size = bytes_left
        .checked_sub(SKIP_SIZE + padding)
        .ok_or(Error::IO(io::Error::new(ErrorKind::InvalidData, "invalid array size")))?;
    let mut slice = alloc
        .alloc(NonZeroUsize::new(opaque_size).unwrap())
        .await
        .ok_or(Error::IO(io::Error::new(ErrorKind::OutOfMemory, "cannot allocate memory")))?;

    let (object, offset, count, mode, size) = parse_with_retry(buffer, socket, write).await?;
    bytes_left -= SKIP_SIZE;
    // is it true???
    assert_eq!(count, size as u32);
    let to_skip = read_in_slice_sync(buffer, &mut slice, size)?;
    read_in_slice_async(socket, &mut slice, to_skip, opaque_size - to_skip).await?;
    bytes_left -= size;
    // all these actions only to read padding

    let pad_in_buf = buffer.available_read();
    buffer.consume(min(pad_in_buf, padding));
    let mut tmp_buf = [0u8, 0u8, 0u8, 0u8];
    socket
        .read_exact(&mut tmp_buf[..ALIGNMENT - min(pad_in_buf, padding)])
        .await
        .map_err(Error::IO)?;
    bytes_left -= padding;
    assert_eq!(bytes_left, 0);
    Ok(WriteArgs { object, offset, count, mode, data: slice })
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
