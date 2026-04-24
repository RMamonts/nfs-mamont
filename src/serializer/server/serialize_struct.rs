//! High-level XDR serializer for complete RPC/NFS replies.
//!
//! This module bridges `crate::vfs` results to the wire format by selecting the
//! appropriate per-procedure serializer from `crate::serializer::nfs` (and
//! mount serializers from `crate::serializer::mount`), then emitting a complete
//! RPC reply to an async writer.

use std::io;
use std::io::{ErrorKind, Write};

use async_trait::async_trait;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::allocator::Slice;
use crate::mount::MountRes;
use crate::rpc::{AcceptStat, Error, OpaqueAuth, RejectedReply, ReplyBody, RpcBody};

use crate::serializer::{u32, usize_as_u32, ALIGNMENT};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::{NfsRes, STATUS_OK};

use super::mount::mnt;
use super::nfs::{
    access, commit, create, error, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node,
    path_conf, read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink,
    write,
};
use super::rpc::auth;

#[async_trait(?Send)]
pub trait WriteSink {
    async fn write_all_bytes(&mut self, buf: &[u8]) -> io::Result<()>;
}

#[async_trait(?Send)]
impl<T> WriteSink for T
where
    T: AsyncWrite + Unpin,
{
    async fn write_all_bytes(&mut self, buf: &[u8]) -> io::Result<()> {
        self.write_all(buf).await
    }
}

/// Minimum buffer size, that could hold complete RPC message
/// with NFSv3 or Mount protocol replies, except for NFSv3 `READ` procedure reply -
/// this size is enough to hold only arguments without opaque data ([`Slice`] in [`crate::vfs::read::Success`])
const DEFAULT_SIZE: usize = 4096;

/// Max size of RMS fragment data
/// (<https://datatracker.ietf.org/doc/html/rfc5531#autoid-19>)
const MAX_FRAGMENT_SIZE: usize = 0x7FFF_FFFF;

/// Header mask of RMS
/// (<https://datatracker.ietf.org/doc/html/rfc5531#autoid-19>)
const HEADER_MASK: usize = 0x8000_0000;

/// Size of RMS header
/// (<https://datatracker.ietf.org/doc/html/rfc5531#autoid-19>)
const HEADER_SIZE: usize = 4;

macro_rules! nfs_result {
    ($self:expr, $res:expr, $ok_fn:path, $fail_fn:path) => {{
        match $res {
            Ok(ok) => {
                usize_as_u32(&mut $self.buffer, STATUS_OK)?;
                $ok_fn(&mut $self.buffer, ok)?;
            }
            Err(err) => {
                error(&mut $self.buffer, err.error)?;
                $fail_fn(&mut $self.buffer, err)?;
            }
        };
        $self.buffer.send_inner_buffer().await
    }};
}

/// Async writer wrapper used to emit XDR-encoded RPC replies.
pub struct Serializer<T: WriteSink> {
    buffer: WriteBuffer<T>,
}

impl<T: WriteSink> Serializer<T> {
    /// Creates a reply serializer writing XDR bytes to the provided async writer.
    pub fn new(writer: T) -> Self {
        Self { buffer: WriteBuffer::new(writer, DEFAULT_SIZE) }
    }

    /// Creates a reply serializer with an explicit internal buffer capacity.
    fn with_capacity(writer: T, capacity: usize) -> Self {
        Self { buffer: WriteBuffer::new(writer, capacity) }
    }

    /// Serializes a [`ProcResult`] into its XDR reply body and writes it to the underlying writer.
    async fn process_result(&mut self, result: ProcResult) -> io::Result<()> {
        match result {
            ProcResult::Nfs3(data) => self.process_nfs3(data).await,
            ProcResult::Mount(data) => self.process_mount(data).await,
        }
    }

    /// Serializes a [`ProcResult::Nfs3`] into its XDR reply body and writes it to the underlying writer.
    async fn process_nfs3(&mut self, data: Box<NfsRes>) -> io::Result<()> {
        match *data {
            NfsRes::Null => self.buffer.send_inner_buffer().await,
            NfsRes::GetAttr(res) => {
                nfs_result!(self, res, get_attr::result_ok, get_attr::result_fail)
            }
            NfsRes::SetAttr(res) => {
                nfs_result!(self, res, set_attr::result_ok, set_attr::result_fail)
            }
            NfsRes::LookUp(res) => {
                nfs_result!(self, res, lookup::result_ok, lookup::result_fail)
            }
            NfsRes::Access(res) => {
                nfs_result!(self, res, access::result_ok, access::result_fail)
            }
            NfsRes::ReadLink(res) => {
                nfs_result!(self, res, read_link::result_ok, read_link::result_fail)
            }
            //special case because of Slice
            NfsRes::Read(res) => match res {
                Ok(ok) => {
                    let count = ok.head.count as usize;
                    usize_as_u32(&mut self.buffer, STATUS_OK)?;
                    read::result_ok_part(&mut self.buffer, ok.head)?;
                    self.buffer.send_inner_with_slice(ok.data, count).await
                }
                Err(err) => {
                    error(&mut self.buffer, err.error)?;
                    read::result_fail(&mut self.buffer, err)?;
                    self.buffer.send_inner_buffer().await
                }
            },
            NfsRes::Write(res) => {
                nfs_result!(self, res, write::result_ok, write::result_fail)
            }
            NfsRes::Create(res) => {
                nfs_result!(self, res, create::result_ok, create::result_fail)
            }
            NfsRes::MkDir(res) => {
                nfs_result!(self, res, mk_dir::result_ok, mk_dir::result_fail)
            }
            NfsRes::SymLink(res) => {
                nfs_result!(self, res, symlink::result_ok, symlink::result_fail)
            }
            NfsRes::MkNod(res) => {
                nfs_result!(self, res, mk_node::result_ok, mk_node::result_fail)
            }
            NfsRes::Remove(res) => {
                nfs_result!(self, res, remove::result_ok, remove::result_fail)
            }
            NfsRes::RmDir(res) => {
                nfs_result!(self, res, rm_dir::result_ok, rm_dir::result_fail)
            }
            NfsRes::Rename(res) => {
                nfs_result!(self, res, rename::result_ok, rename::result_fail)
            }
            NfsRes::Link(res) => {
                nfs_result!(self, res, link::result_ok, link::result_fail)
            }
            NfsRes::ReadDir(res) => {
                nfs_result!(self, res, read_dir::result_ok, read_dir::result_fail)
            }
            NfsRes::ReadDirPlus(res) => {
                nfs_result!(self, res, read_dir_plus::result_ok, read_dir_plus::result_fail)
            }
            NfsRes::FsStat(res) => {
                nfs_result!(self, res, fs_stat::result_ok, fs_stat::result_fail)
            }
            NfsRes::FsInfo(res) => {
                nfs_result!(self, res, fs_info::result_ok, fs_info::result_fail)
            }
            NfsRes::PathConf(res) => {
                nfs_result!(self, res, path_conf::result_ok, path_conf::result_fail)
            }
            NfsRes::Commit(res) => {
                nfs_result!(self, res, commit::result_ok, commit::result_fail)
            }
        }
    }

    /// Serializes a [`ProcResult::Mount`] into its XDR reply body and writes it to the underlying writer.
    async fn process_mount(&mut self, data: Box<MountRes>) -> io::Result<()> {
        match *data {
            MountRes::Null | MountRes::UnmountAll | MountRes::Unmount => {
                self.buffer.send_inner_buffer().await
            }
            MountRes::Mount(res) => {
                match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        mnt::result_ok(&mut self.buffer, ok)?;
                    }
                    Err(stat) => {
                        super::mount::mount_stat(&mut self.buffer, stat)?;
                    }
                };
                self.buffer.send_inner_buffer().await
            }
            MountRes::Export(node) => {
                super::mount::export::result_ok(&mut self.buffer, node)?;
                self.buffer.send_inner_buffer().await
            }
            MountRes::Dump(body) => {
                super::mount::dump::result_ok(&mut self.buffer, body)?;
                self.buffer.send_inner_buffer().await
            }
        }
    }

    /// Serializes [`ProcReply`] into a complete XDR RPC reply and writes it to the underlying writer.
    ///
    /// ## Arguments:
    /// *   `reply` - procedure result of [`ProcReply`] type
    /// *   `verifier` - an authentication verifier of [`OpaqueAuth`] type that the server generates in
    ///     order to validate itself to the client
    ///
    /// TODO:(<https://github.com/RMamonts/nfs-mamont/issues/137>)
    pub async fn form_reply(&mut self, reply: ProcReply, verifier: OpaqueAuth) -> io::Result<()> {
        u32(&mut self.buffer, reply.xid)?;
        u32(&mut self.buffer, RpcBody::Reply as u32)?;
        match reply.proc_result {
            Ok(proc) => {
                u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                auth(&mut self.buffer, verifier)?;
                u32(&mut self.buffer, AcceptStat::Success as u32)?;
                self.process_result(proc).await
            }
            Err(err) => {
                match err {
                    Error::ImpossibleTypeCast
                    | Error::BadFileHandle
                    | Error::MessageTypeMismatch
                    | Error::EnumDiscMismatch
                    | Error::MaxElemLimit
                    | Error::IncorrectString(_) => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        // or maybe system error?
                        u32(&mut self.buffer, AcceptStat::GarbageArgs as u32)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Error::RpcVersionMismatch(vers) => {
                        u32(&mut self.buffer, ReplyBody::MsgDenied as u32)?;
                        u32(&mut self.buffer, RejectedReply::RpcMismatch as u32)?;
                        u32(&mut self.buffer, vers.low)?;
                        u32(&mut self.buffer, vers.high)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Error::Auth(stat) => {
                        u32(&mut self.buffer, ReplyBody::MsgDenied as u32)?;
                        u32(&mut self.buffer, RejectedReply::AuthError as u32)?;
                        u32(&mut self.buffer, stat as u32)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Error::ProgramMismatch => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        u32(&mut self.buffer, AcceptStat::ProgUnavail as u32)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Error::ProcedureMismatch => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        u32(&mut self.buffer, AcceptStat::ProcUnavail as u32)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Error::ProgramVersionMismatch(info) => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        u32(&mut self.buffer, AcceptStat::ProgMismatch as u32)?;
                        u32(&mut self.buffer, info.low)?;
                        u32(&mut self.buffer, info.high)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Error::IO(_) => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        u32(&mut self.buffer, AcceptStat::SystemErr as u32)?;
                        self.buffer.send_inner_buffer().await
                    }
                }
            }
        }
    }
}

/// Buffered async writer used by the high-level reply serializer.
struct WriteBuffer<T: WriteSink> {
    socket: T,
    buf: Vec<u8>,
}

impl<T: WriteSink> Write for WriteBuffer<T> {
    /// Writes raw bytes into the internal staging buffer (not directly to the socket).
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    /// No-op flush (the buffer is flushed explicitly by `send_inner_*`).
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<T: WriteSink> WriteBuffer<T> {
    /// Creates a new buffer around an async writer with a fixed preallocated capacity.
    fn new(socket: T, capacity: usize) -> WriteBuffer<T> {
        let mut buffer = WriteBuffer { socket, buf: Vec::with_capacity(capacity) };
        buffer.clean();
        buffer
    }

    /// Resets the internal write cursor to the start of the buffer.
    fn clean(&mut self) {
        self.buf.clear();
        // reserve first 4 bytes to write header by RMS
        // https://datatracker.ietf.org/doc/html/rfc5531#autoid-19
        self.buf.extend_from_slice(&[0, 0, 0, 0]);
    }

    fn append_fragment_size(&mut self, size: usize) -> io::Result<()> {
        // now only single RMS fragment is allowed
        // TODO(https://github.com/RMamonts/nfs-mamont/issues/103)
        if size > MAX_FRAGMENT_SIZE {
            return Err(io::Error::new(
                ErrorKind::Unsupported,
                "Fragmented messages not supported",
            ));
        }
        // there is no need for check, since we initialize vector in new()
        // and we append 4 bytes after clean()
        // since we check size for MAX_FRAGMENT_SIZE (which is less than u32::MAX) cast is safe
        self.buf[..HEADER_SIZE].copy_from_slice(&((HEADER_MASK | size) as u32).to_be_bytes());
        Ok(())
    }

    /// Flushes the staged XDR bytes to the underlying writer.
    async fn send_inner_buffer(&mut self) -> io::Result<()> {
        self.append_fragment_size(self.buf.len().saturating_sub(HEADER_SIZE))?;
        self.socket.write_all_bytes(&self.buf).await?;
        self.clean();
        Ok(())
    }

    /// Flushes the staged XDR bytes followed by a streamed payload [`Slice`] (used for READ data).
    async fn send_inner_with_slice(&mut self, slice: Slice, count: usize) -> io::Result<()> {
        // this place is a bit paradox
        // In READ procedure (https://datatracker.ietf.org/doc/html/rfc1813#autoid-25) opaque data
        // (which is represented with Slice in vfs::read::Success) from XDR
        // (https://datatracker.ietf.org/doc/html/rfc4506#autoid-17) is used, which mean (quote):
        //    The array is encoded as the element count n
        //    (an unsigned integer) followed by the encoding of each of the array's
        //    elements, starting with element 0 and progressing through element n-1.
        // so we need to pass size of Slice before actual opaque data
        u32(&mut self.buf, count as u32)?;

        let padding = (ALIGNMENT - count % ALIGNMENT) % ALIGNMENT;
        self.append_fragment_size(self.buf.len().saturating_sub(HEADER_SIZE) + count + padding)?;
        self.socket.write_all_bytes(&self.buf).await?;

        // later change to explicit cursor (when one implemented)
        for buf in slice.iter() {
            self.socket.write_all_bytes(buf).await?;
        }

        // write padding directly to socket
        let slice = [0u8; ALIGNMENT];
        self.socket.write_all_bytes(&slice[..padding]).await?;
        self.clean();
        Ok(())
    }
}
