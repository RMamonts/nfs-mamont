//! High-level XDR serializer for complete RPC/NFS replies.
//!
//! This module bridges `crate::vfs` results to the wire format by selecting the
//! appropriate per-procedure serializer from `crate::serializer::nfs` (and
//! mount serializers from `crate::serializer::mount`), then emitting a complete
//! RPC reply to an async writer.

use std::io;
use std::io::{ErrorKind, IoSlice, Write};

use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::allocator::Buffer;
use crate::mount::MountRes;
use crate::nlm::NlmRes;
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
use super::nlm;
use super::rpc::auth;

/// Minimum buffer size, that could hold complete RPC message
/// with NFSv3 or Mount protocol replies, except for NFSv3 `READ` procedure reply -
/// this size is enough to hold only arguments without opaque data ([`Buffer`] in [`crate::vfs::read::Success`])
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
        $self.buffer.finish_reply()
    }};
}

/// Async writer wrapper used to emit XDR-encoded RPC replies.
pub struct Serializer<B: Buffer, T: AsyncWrite + Unpin> {
    buffer: WriteBuffer<B, T>,
}

impl<B: Buffer, T: AsyncWrite + Unpin> Serializer<B, T> {
    /// Creates a reply serializer writing XDR bytes to the provided async writer.
    pub fn new(writer: T) -> Self {
        Self { buffer: WriteBuffer::new(writer, DEFAULT_SIZE) }
    }

    /// Creates a reply serializer with an explicit internal buffer capacity.
    #[allow(dead_code)]
    fn with_capacity(writer: T, capacity: usize) -> Self {
        Self { buffer: WriteBuffer::new(writer, capacity) }
    }

    /// Writes all staged replies to the underlying writer with a single write.
    pub async fn flush(&mut self) -> io::Result<()> {
        self.buffer.flush().await
    }

    /// Number of staged bytes not yet written to the underlying writer.
    pub fn buffered_len(&self) -> usize {
        self.buffer.buffered_len()
    }

    /// Drops all staged bytes. Used after a write error to avoid sending
    /// a partially serialized (corrupt) byte stream.
    pub fn reset(&mut self) {
        self.buffer.reset();
    }

    /// Truncates the staging buffer back to `len` bytes.
    ///
    /// Used to drop a partially staged reply after [`Serializer::form_reply`]
    /// fails, while keeping the previously staged (complete) replies intact.
    pub fn truncate_staged(&mut self, len: usize) {
        self.buffer.truncate(len);
    }

    /// Serializes a [`ProcResult`] into its XDR reply body and writes it to the underlying writer.
    async fn process_result(&mut self, result: ProcResult<B>) -> io::Result<()> {
        match result {
            ProcResult::Nfs3(data) => self.process_nfs3(data).await,
            ProcResult::Mount(data) => self.process_mount(data).await,
            ProcResult::Nlm4(data) => self.process_nlm(data).await,
        }
    }

    /// Serializes a [`ProcResult::Nfs3`] into its XDR reply body and writes it to the underlying writer.
    async fn process_nfs3(&mut self, data: Box<NfsRes<B>>) -> io::Result<()> {
        match *data {
            NfsRes::Null => self.buffer.finish_reply(),
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
            NfsRes::Read(res) => match res {
                Ok(ok) => {
                    let count = ok.head.count as usize;
                    usize_as_u32(&mut self.buffer, STATUS_OK)?;
                    read::result_ok_part(&mut self.buffer, ok.head)?;
                    self.buffer.finish_reply_with_payload(ok.data, count).await
                }
                Err(err) => {
                    error(&mut self.buffer, err.error)?;
                    read::result_fail(&mut self.buffer, err)?;
                    self.buffer.finish_reply()
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

    /// Serializes a [`ProcResult::Nlm4`] into its XDR reply body and writes it to the underlying writer.
    async fn process_nlm(&mut self, data: Box<NlmRes>) -> io::Result<()> {
        match *data {
            NlmRes::Null => self.buffer.finish_reply(),
            NlmRes::Lock(res) => {
                nlm::lock_res(&mut self.buffer, res)?;
                self.buffer.finish_reply()
            }
            NlmRes::Unlock(res) => {
                nlm::unlock_res(&mut self.buffer, res)?;
                self.buffer.finish_reply()
            }
            NlmRes::Test(res) => {
                nlm::test_res(&mut self.buffer, *res)?;
                self.buffer.finish_reply()
            }
            NlmRes::Cancel(res) => {
                nlm::cancel_res(&mut self.buffer, res)?;
                self.buffer.finish_reply()
            }
        }
    }

    /// Serializes a [`ProcResult::Mount`] into its XDR reply body and writes it to the underlying writer.
    async fn process_mount(&mut self, data: Box<MountRes>) -> io::Result<()> {
        match *data {
            MountRes::Null | MountRes::UnmountAll | MountRes::Unmount => self.buffer.finish_reply(),
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
                self.buffer.finish_reply()
            }
            MountRes::Export(node) => {
                super::mount::export::result_ok(&mut self.buffer, node)?;
                self.buffer.finish_reply()
            }
            MountRes::Dump(body) => {
                super::mount::dump::result_ok(&mut self.buffer, body)?;
                self.buffer.finish_reply()
            }
        }
    }

    /// Serializes [`ProcReply`] into a complete XDR RPC reply.
    ///
    /// The reply is staged into the internal buffer; call [`Serializer::flush`]
    /// to actually write the staged bytes to the underlying writer. This allows
    /// batching several replies into a single socket write. The only exception
    /// is a successful NFSv3 `READ` reply: its opaque payload is written
    /// immediately (together with all previously staged replies) using a single
    /// vectored write, so after it the internal buffer is empty.
    ///
    /// ## Arguments:
    /// *   `reply` - procedure result of [`ProcReply`] type
    /// *   `verifier` - an authentication verifier of [`OpaqueAuth`] type that the server generates in
    ///     order to validate itself to the client
    ///
    /// TODO:(<https://github.com/RMamonts/nfs-mamont/issues/137>)
    pub async fn form_reply(
        &mut self,
        reply: ProcReply<B>,
        verifier: OpaqueAuth,
    ) -> io::Result<()> {
        self.buffer.begin_reply();
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
                        self.buffer.finish_reply()
                    }
                    Error::RpcVersionMismatch(vers) => {
                        u32(&mut self.buffer, ReplyBody::MsgDenied as u32)?;
                        u32(&mut self.buffer, RejectedReply::RpcMismatch as u32)?;
                        u32(&mut self.buffer, vers.low)?;
                        u32(&mut self.buffer, vers.high)?;
                        self.buffer.finish_reply()
                    }
                    Error::Auth(stat) => {
                        u32(&mut self.buffer, ReplyBody::MsgDenied as u32)?;
                        u32(&mut self.buffer, RejectedReply::AuthError as u32)?;
                        u32(&mut self.buffer, stat as u32)?;
                        self.buffer.finish_reply()
                    }
                    Error::ProgramMismatch => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        u32(&mut self.buffer, AcceptStat::ProgUnavail as u32)?;
                        self.buffer.finish_reply()
                    }
                    Error::ProcedureMismatch => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        u32(&mut self.buffer, AcceptStat::ProcUnavail as u32)?;
                        self.buffer.finish_reply()
                    }
                    Error::ProgramVersionMismatch(info) => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        u32(&mut self.buffer, AcceptStat::ProgMismatch as u32)?;
                        u32(&mut self.buffer, info.low)?;
                        u32(&mut self.buffer, info.high)?;
                        self.buffer.finish_reply()
                    }
                    Error::IO(_) => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        u32(&mut self.buffer, AcceptStat::SystemErr as u32)?;
                        self.buffer.finish_reply()
                    }
                }
            }
        }
    }
}

/// Buffered async writer used by the high-level reply serializer.
///
/// Several replies may be staged into the internal buffer before a single
/// `flush` writes them all to the socket with one syscall. Each staged reply
/// starts with a 4-byte RMS header placeholder (reserved by [`Self::begin_reply`])
/// that is patched by [`Self::finish_reply`] once the reply size is known.
struct WriteBuffer<B: Buffer, T: AsyncWrite + Unpin> {
    socket: T,
    buf: Vec<u8>,
    /// Offset of the RMS header of the reply currently being serialized.
    reply_start: usize,
    _phantom: std::marker::PhantomData<B>,
}

impl<B: Buffer, T: AsyncWrite + Unpin> Write for WriteBuffer<B, T> {
    /// Writes raw bytes into the internal staging buffer (not directly to the socket).
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    /// No-op flush (the buffer is flushed explicitly by `flush`).
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<B: Buffer, T: AsyncWrite + Unpin> WriteBuffer<B, T> {
    /// Creates a new buffer around an async writer with a fixed preallocated capacity.
    fn new(socket: T, capacity: usize) -> WriteBuffer<B, T> {
        WriteBuffer {
            socket,
            buf: Vec::with_capacity(capacity),
            reply_start: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Starts a new reply: remembers its offset and reserves 4 bytes
    /// for the RMS header
    /// (<https://datatracker.ietf.org/doc/html/rfc5531#autoid-19>).
    fn begin_reply(&mut self) {
        self.reply_start = self.buf.len();
        self.buf.extend_from_slice(&[0, 0, 0, 0]);
    }

    /// Patches the RMS header of the current reply at `reply_start`.
    fn append_fragment_size(&mut self, size: usize) -> io::Result<()> {
        // now only single RMS fragment is allowed
        // TODO(https://github.com/RMamonts/nfs-mamont/issues/103)
        if size > MAX_FRAGMENT_SIZE {
            return Err(io::Error::new(
                ErrorKind::Unsupported,
                "Fragmented messages not supported",
            ));
        }
        // the 4 bytes at reply_start were reserved by begin_reply();
        // since we check size for MAX_FRAGMENT_SIZE (which is less than u32::MAX) cast is safe
        self.buf[self.reply_start..self.reply_start + HEADER_SIZE]
            .copy_from_slice(&((HEADER_MASK | size) as u32).to_be_bytes());
        Ok(())
    }

    /// Completes the current reply in the staging buffer without any socket I/O.
    ///
    /// The reply stays staged until [`Self::flush`] is called, allowing several
    /// replies to be coalesced into a single socket write.
    fn finish_reply(&mut self) -> io::Result<()> {
        let size = self.buf.len() - self.reply_start - HEADER_SIZE;
        self.append_fragment_size(size)
    }

    /// Writes all staged bytes to the underlying writer and clears the buffer.
    async fn flush(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        self.socket.write_all(&self.buf).await?;
        self.buf.clear();
        Ok(())
    }

    /// Number of staged bytes not yet written to the socket.
    fn buffered_len(&self) -> usize {
        self.buf.len()
    }

    /// Drops all staged bytes without writing them.
    fn reset(&mut self) {
        self.buf.clear();
    }

    /// Truncates the staging buffer back to `len` bytes.
    fn truncate(&mut self, len: usize) {
        self.buf.truncate(len);
    }

    /// Completes the current reply and immediately writes all staged bytes
    /// followed by a streamed payload [`Buffer`] (used for READ data).
    ///
    /// Uses vectored I/O to coalesce the staged replies, all payload chunks and
    /// padding into a single `writev`-style syscall, reducing kernel transitions.
    async fn finish_reply_with_payload(&mut self, buffer: B, count: usize) -> io::Result<()> {
        // this place is a bit paradox
        // In READ procedure (https://datatracker.ietf.org/doc/html/rfc1813#autoid-25) opaque data
        // (which is represented with Buffer in vfs::read::Success) from XDR
        // (https://datatracker.ietf.org/doc/html/rfc4506#autoid-17) is used, which mean (quote):
        //    The array is encoded as the element count n
        //    (an unsigned integer) followed by the encoding of each of the array's
        //    elements, starting with element 0 and progressing through element n-1.
        // so we need to pass size of buffer before actual opaque data
        u32(&mut self.buf, count as u32)?;

        let padding = (ALIGNMENT - count % ALIGNMENT) % ALIGNMENT;
        let staged_size = self.buf.len() - self.reply_start - HEADER_SIZE;
        self.append_fragment_size(staged_size + count + padding)?;

        let padding_bytes = [0u8; ALIGNMENT];

        let mut written: usize = 0;
        let total_payload: usize = self.buf.len() + buffer.len() + padding;

        let mut iov: Vec<IoSlice<'_>> = Vec::with_capacity(buffer.chunks().count() + 2);

        while written < total_payload {
            iov.clear();
            let mut to_skip = written;

            // Staged replies (including the head of the current one) go first,
            // so the whole batch and the payload leave in one vectored write.
            if to_skip < self.buf.len() {
                iov.push(IoSlice::new(&self.buf[to_skip..]));
                to_skip = 0;
            } else {
                to_skip -= self.buf.len();
            }

            for chunk in buffer.chunks() {
                if to_skip == 0 {
                    iov.push(IoSlice::new(chunk));
                } else if to_skip < chunk.len() {
                    iov.push(IoSlice::new(&chunk[to_skip..]));
                    to_skip = 0;
                } else {
                    to_skip -= chunk.len();
                }
            }

            if to_skip < padding {
                iov.push(IoSlice::new(&padding_bytes[to_skip..padding]));
            }

            if !iov.is_empty() {
                let n = self.socket.write_vectored(&iov).await?;
                if n == 0 {
                    return Err(io::Error::new(
                        ErrorKind::WriteZero,
                        "failed to write data to socket",
                    ));
                }
                written += n;
            } else {
                break;
            }
        }

        self.buf.clear();
        Ok(())
    }
}
