//! High-level XDR serializer for complete RPC/NFS replies.
//!
//! This module bridges `crate::vfs` results to the wire format by selecting the
//! appropriate per-procedure serializer from `crate::serializer::nfs` (and
//! mount serializers from `crate::serializer::mount`), then emitting a complete
//! RPC reply to an async writer. Replies are staged and sent in batches.

use std::io;
use std::io::{ErrorKind, IoSlice, Write};
use std::{iter, mem};

use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::allocator::Buffer;
use crate::consts::rpc::{HEADER_MASK, MAX_FRAGMENT_SIZE, RMS_HEADER_SIZE};
use crate::mount::MountRes;
use crate::nlm::NlmRes;
use crate::rpc::{AcceptStat, Error, OpaqueAuth, RejectedReply, ReplyBody, RpcBody};

use crate::serializer::{padding, u32, usize_as_u32};
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
        Ok(())
    }};
}

/// Async writer wrapper used to emit XDR-encoded RPC replies.
///
/// Replies are staged by [`Serializer::form_reply`] and sent together by [`Serializer::flush`].
pub struct Serializer<T: AsyncWrite + Unpin, B: Buffer> {
    buffer: WriteBuffer<T, B>,
}

impl<T: AsyncWrite + Unpin, B: Buffer> Serializer<T, B> {
    /// Creates a reply serializer writing XDR bytes to the provided async writer.
    pub fn new(writer: T) -> Self {
        Self::with_capacity(writer, DEFAULT_SIZE)
    }

    /// Creates a reply serializer with an explicit internal buffer capacity.
    fn with_capacity(writer: T, capacity: usize) -> Self {
        Self { buffer: WriteBuffer::new(writer, capacity) }
    }

    /// Serializes a [`ProcResult`] into its XDR reply body and writes it to the underlying writer.
    fn process_result(&mut self, result: ProcResult<B>) -> io::Result<()> {
        match result {
            ProcResult::Nfs3(data) => self.process_nfs3(*data),
            ProcResult::Mount(data) => self.process_mount(*data),
            ProcResult::Nlm4(data) => self.process_nlm(*data),
        }
    }

    /// Serializes a [`ProcResult::Nfs3`] into its XDR reply body and writes it to the underlying writer.
    fn process_nfs3(&mut self, data: NfsRes<B>) -> io::Result<()> {
        match data {
            NfsRes::Null => Ok(()),
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
                    self.buffer.push_payload(ok.data, count)
                }
                Err(err) => {
                    error(&mut self.buffer, err.error)?;
                    read::result_fail(&mut self.buffer, err)?;
                    Ok(())
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
    fn process_nlm(&mut self, data: NlmRes) -> io::Result<()> {
        match data {
            NlmRes::Null => {}
            NlmRes::Lock(res) => nlm::lock_res(&mut self.buffer, res)?,
            NlmRes::Unlock(res) => nlm::unlock_res(&mut self.buffer, res)?,
            NlmRes::Test(res) => nlm::test_res(&mut self.buffer, *res)?,
            NlmRes::Cancel(res) => nlm::cancel_res(&mut self.buffer, res)?,
        }
        Ok(())
    }

    /// Serializes a [`ProcResult::Mount`] into its XDR reply body and writes it to the underlying writer.
    fn process_mount(&mut self, data: MountRes) -> io::Result<()> {
        match data {
            MountRes::Null | MountRes::UnmountAll | MountRes::Unmount => {}
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
            }
            MountRes::Export(node) => {
                super::mount::export::result_ok(&mut self.buffer, node)?;
            }
            MountRes::Dump(body) => {
                super::mount::dump::result_ok(&mut self.buffer, body)?;
            }
        };
        Ok(())
    }

    /// Serializes [`ProcReply`] into a complete XDR RPC reply and stages it until [`Serializer::flush`].
    /// On error nothing of the reply stays staged.
    ///
    /// ## Arguments:
    /// *   `reply` - procedure result of [`ProcReply`] type
    /// *   `verifier` - an authentication verifier of [`OpaqueAuth`] type that the server generates in
    ///     order to validate itself to the client
    ///
    /// TODO:(<https://github.com/RMamonts/nfs-mamont/issues/137>)
    pub fn form_reply(&mut self, reply: ProcReply<B>, verifier: OpaqueAuth) -> io::Result<()> {
        self.buffer.begin_record();
        let result = self.serialize(reply, verifier).and_then(|()| self.buffer.end_record());
        if result.is_err() {
            self.buffer.discard_record();
        }
        result
    }

    /// Sends all staged replies.
    pub async fn flush(&mut self) -> io::Result<()> {
        self.buffer.flush().await
    }

    /// Bytes staged for the next [`Serializer::flush`], payloads included.
    pub fn staged_len(&self) -> usize {
        self.buffer.staged_len()
    }

    fn serialize(&mut self, reply: ProcReply<B>, verifier: OpaqueAuth) -> io::Result<()> {
        u32(&mut self.buffer, reply.xid)?;
        u32(&mut self.buffer, RpcBody::Reply as u32)?;
        match reply.proc_result {
            Ok(proc) => {
                u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                auth(&mut self.buffer, verifier)?;
                u32(&mut self.buffer, AcceptStat::Success as u32)?;
                self.process_result(proc)
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
                        Ok(())
                    }
                    Error::RpcVersionMismatch(vers) => {
                        u32(&mut self.buffer, ReplyBody::MsgDenied as u32)?;
                        u32(&mut self.buffer, RejectedReply::RpcMismatch as u32)?;
                        u32(&mut self.buffer, vers.low)?;
                        u32(&mut self.buffer, vers.high)?;
                        Ok(())
                    }
                    Error::Auth(stat) => {
                        u32(&mut self.buffer, ReplyBody::MsgDenied as u32)?;
                        u32(&mut self.buffer, RejectedReply::AuthError as u32)?;
                        u32(&mut self.buffer, stat as u32)?;
                        Ok(())
                    }
                    Error::ProgramMismatch => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        u32(&mut self.buffer, AcceptStat::ProgUnavail as u32)?;
                        Ok(())
                    }
                    Error::ProcedureMismatch => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        u32(&mut self.buffer, AcceptStat::ProcUnavail as u32)?;
                        Ok(())
                    }
                    Error::ProgramVersionMismatch(info) => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        u32(&mut self.buffer, AcceptStat::ProgMismatch as u32)?;
                        u32(&mut self.buffer, info.low)?;
                        u32(&mut self.buffer, info.high)?;
                        Ok(())
                    }
                    Error::IO(_) => {
                        u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                        auth(&mut self.buffer, verifier)?;
                        u32(&mut self.buffer, AcceptStat::SystemErr as u32)?;
                        Ok(())
                    }
                }
            }
        }
    }
}

/// Max iovecs per `writev`, as in tokio's `write_all_buf`; the rest goes in the next call.
const MAX_IOV: usize = 64;

/// Bulk payload (READ data) spliced into the staged bytes at `offset`.
struct Payload<B> {
    offset: usize,
    data: B,
    /// Only the first `count` bytes of `data` are part of the reply.
    count: usize,
}

/// Buffered async writer used by the high-level reply serializer.
///
/// Stages several complete RPC records: their bytes in `buf`, except for READ
/// payloads, which are referenced from `payloads` instead of being copied.
struct WriteBuffer<T: AsyncWrite + Unpin, B: Buffer> {
    socket: T,
    buf: Vec<u8>,
    payloads: Vec<Payload<B>>,
    /// `buf` and `payloads` lengths at the start of the record being serialized.
    record: (usize, usize),
}

impl<T: AsyncWrite + Unpin, B: Buffer> Write for WriteBuffer<T, B> {
    /// Writes raw bytes into the internal staging buffer (not directly to the socket).
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    /// No-op flush (the buffer is flushed explicitly by [`WriteBuffer::flush`]).
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<T: AsyncWrite + Unpin, B: Buffer> WriteBuffer<T, B> {
    /// Creates a new buffer around an async writer with an initial capacity.
    fn new(socket: T, capacity: usize) -> Self {
        WriteBuffer {
            socket,
            buf: Vec::with_capacity(capacity),
            payloads: Vec::new(),
            record: (0, 0),
        }
    }

    /// Starts a new record.
    fn begin_record(&mut self) {
        self.record = (self.buf.len(), self.payloads.len());
        // reserve 4 bytes to write header by RMS
        // https://datatracker.ietf.org/doc/html/rfc5531#autoid-19
        self.buf.extend_from_slice(&[0; RMS_HEADER_SIZE]);
    }

    /// Writes the RMS header of the current record.
    fn end_record(&mut self) -> io::Result<()> {
        let (start, first_payload) = self.record;
        let payload: usize = self.payloads[first_payload..].iter().map(|p| p.count).sum();
        let size = self.buf.len() - start - RMS_HEADER_SIZE + payload;
        // now only single RMS fragment is allowed
        // TODO(https://github.com/RMamonts/nfs-mamont/issues/103)
        if size > MAX_FRAGMENT_SIZE {
            return Err(io::Error::new(
                ErrorKind::Unsupported,
                "Fragmented messages not supported",
            ));
        }
        // since we check size for MAX_FRAGMENT_SIZE (which is less than u32::MAX) cast is safe
        self.buf[start..start + RMS_HEADER_SIZE]
            .copy_from_slice(&((HEADER_MASK | size) as u32).to_be_bytes());
        Ok(())
    }

    /// Drops the current, partially serialized record.
    fn discard_record(&mut self) {
        let (start, first_payload) = self.record;
        self.buf.truncate(start);
        self.payloads.truncate(first_payload);
    }

    /// Stages `count` bytes of the payload [`Buffer`] (used for READ data) as XDR opaque data.
    fn push_payload(&mut self, data: B, count: usize) -> io::Result<()> {
        if count > data.len() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "reply declares more payload bytes than the buffer holds",
            ));
        }

        // this place is a bit paradox
        // In READ procedure (https://datatracker.ietf.org/doc/html/rfc1813#autoid-25) opaque data
        // (which is represented with Buffer in vfs::read::Success) from XDR
        // (https://datatracker.ietf.org/doc/html/rfc4506#autoid-17) is used, which mean (quote):
        //    The array is encoded as the element count n
        //    (an unsigned integer) followed by the encoding of each of the array's
        //    elements, starting with element 0 and progressing through element n-1.
        // so we need to pass size of buffer before actual opaque data
        u32(&mut self.buf, count as u32)?;
        self.payloads.push(Payload { offset: self.buf.len(), data, count });
        // padding goes out with the bytes that follow the payload
        padding(&mut self.buf, count)
    }

    /// Bytes staged for the next [`WriteBuffer::flush`], payloads included.
    fn staged_len(&self) -> usize {
        self.buf.len() + self.payloads.iter().map(|p| p.count).sum::<usize>()
    }

    /// Sends all staged records with vectored writes and clears them, even on error,
    /// so that payload buffers are returned to the allocator.
    async fn flush(&mut self) -> io::Result<()> {
        let result = self.write_staged().await;
        self.buf.clear();
        self.payloads.clear();
        result
    }

    async fn write_staged(&mut self) -> io::Result<()> {
        let total = self.staged_len();
        let mut written = 0;
        while written < total {
            let mut iov = [IoSlice::new(&[]); MAX_IOV];
            let mut len = 0;
            // partial writes are rare, so resume by skipping what was already written
            let mut skip = written;
            for segment in segments(&self.buf, &self.payloads) {
                if skip >= segment.len() {
                    skip -= segment.len();
                    continue;
                }
                iov[len] = IoSlice::new(&segment[skip..]);
                skip = 0;
                len += 1;
                if len == MAX_IOV {
                    break;
                }
            }

            let n = self.socket.write_vectored(&iov[..len]).await?;
            if n == 0 {
                return Err(io::Error::new(ErrorKind::WriteZero, "failed to write data to socket"));
            }
            written += n;
        }
        Ok(())
    }
}

/// Staged bytes in send order: `buf` with each payload spliced in at its offset.
fn segments<'a, B: Buffer>(
    buf: &'a [u8],
    payloads: &'a [Payload<B>],
) -> impl Iterator<Item = &'a [u8]> {
    let tail = payloads.last().map_or(0, |p| p.offset);
    let mut start = 0;
    payloads
        .iter()
        .flat_map(move |p| {
            let head = &buf[mem::replace(&mut start, p.offset)..p.offset];
            let mut left = p.count;
            let data = p.data.chunks().map(move |chunk| {
                let chunk = &chunk[..chunk.len().min(left)];
                left -= chunk.len();
                chunk
            });
            iter::once(head).chain(data)
        })
        .chain(iter::once(&buf[tail..]))
        .filter(|segment| !segment.is_empty())
}

#[cfg(test)]
#[path = "serialize_struct_tests.rs"]
mod serialize_struct_tests;
