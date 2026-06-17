//! High-level XDR serializer for complete RPC/NFS replies.
//!
//! This module bridges `crate::vfs` results to the wire format by selecting the
//! appropriate per-procedure serializer from `crate::serializer::nfs` (and
//! mount serializers from `crate::serializer::mount`), then emitting a complete
//! RPC reply to an async writer.

use std::io;
use std::io::Write;

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
            NfsRes::Read(res) => match res {
                Ok(ok) => {
                    let count = ok.head.count as usize;
                    usize_as_u32(&mut self.buffer, STATUS_OK)?;
                    read::result_ok_part(&mut self.buffer, ok.head)?;
                    self.buffer.send_inner_with_buffer(ok.data, count).await
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

    /// Serializes a [`ProcResult::Nlm4`] into its XDR reply body and writes it to the underlying writer.
    async fn process_nlm(&mut self, data: Box<NlmRes>) -> io::Result<()> {
        match *data {
            NlmRes::Null => self.buffer.send_inner_buffer().await,
            NlmRes::Lock(res) => {
                nlm::lock_res(&mut self.buffer, res)?;
                self.buffer.send_inner_buffer().await
            }
            NlmRes::Unlock(res) => {
                nlm::unlock_res(&mut self.buffer, res)?;
                self.buffer.send_inner_buffer().await
            }
            NlmRes::Test(res) => {
                nlm::test_res(&mut self.buffer, *res)?;
                self.buffer.send_inner_buffer().await
            }
            NlmRes::Cancel(res) => {
                nlm::cancel_res(&mut self.buffer, res)?;
                self.buffer.send_inner_buffer().await
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
    pub async fn form_reply(
        &mut self,
        reply: ProcReply<B>,
        verifier: OpaqueAuth,
    ) -> io::Result<()> {
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
struct WriteBuffer<B: Buffer, T: AsyncWrite + Unpin> {
    socket: T,
    buf: Vec<u8>,
    _phantom: std::marker::PhantomData<B>,
}

impl<B: Buffer, T: AsyncWrite + Unpin> Write for WriteBuffer<B, T> {
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

impl<B: Buffer, T: AsyncWrite + Unpin> WriteBuffer<B, T> {
    /// Creates a new buffer around an async writer with a fixed preallocated capacity.
    fn new(socket: T, capacity: usize) -> WriteBuffer<B, T> {
        WriteBuffer {
            socket,
            buf: Vec::with_capacity(capacity),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Resets the internal write buffer.
    fn clean(&mut self) {
        self.buf.clear();
    }

    /// Flushes the staged XDR bytes to the underlying writer as one or more RMS fragments.
    async fn send_inner_buffer(&mut self) -> io::Result<()> {
        let buf = std::mem::take(&mut self.buf);
        let mut fw = FragmentedWriter::new(&mut self.socket, buf.len()).await?;
        fw.write_bytes(&buf).await?;
        self.buf = buf;
        self.clean();
        Ok(())
    }

    /// Flushes the staged XDR bytes followed by a streamed payload [`Buffer`] (used for READ data).
    ///
    /// Per the READ procedure in [RFC 1813 §3.3.6](https://datatracker.ietf.org/doc/html/rfc1813#autoid-25),
    /// the opaque data is XDR variable-length opaque: the element count `n` is encoded first
    /// (as a `u32`), followed by the `n` data bytes. The count is written into the staging buffer
    /// so that it appears on the wire immediately before the file data.
    async fn send_inner_with_buffer(&mut self, buffer: B, count: usize) -> io::Result<()> {
        u32(&mut self.buf, count as u32)?;

        let padding_size = (ALIGNMENT - count % ALIGNMENT) % ALIGNMENT;
        let buf = std::mem::take(&mut self.buf);
        let total = buf.len() + count + padding_size;

        let mut fw = FragmentedWriter::new(&mut self.socket, total).await?;
        fw.write_bytes(&buf).await?;

        for chunk in buffer.chunks() {
            fw.write_bytes(chunk).await?;
        }

        let padding_bytes = [0u8; ALIGNMENT];
        fw.write_bytes(&padding_bytes[..padding_size]).await?;

        self.buf = buf;
        self.clean();
        Ok(())
    }
}

/// Streams data to an async writer as one or more RMS-framed fragments.
///
/// Per [RFC 5531 §11](https://datatracker.ietf.org/doc/html/rfc5531#autoid-19), each RPC message
/// sent over TCP is a *record* consisting of one or more *fragments*. Each fragment is a 4-byte
/// big-endian header followed by the fragment data:
///
/// - bit 31 of the header is set on the **last** fragment of the record
/// - bits 30..0 hold the fragment data byte count
///
/// [`FragmentedWriter::new`] writes the first fragment header to the socket immediately.
/// Subsequent headers are emitted automatically by [`FragmentedWriter::write_bytes`] whenever a
/// fragment boundary is crossed. The total bytes supplied via [`write_bytes`] must equal the
/// `total` argument given to the constructor.
struct FragmentedWriter<'a, T: AsyncWrite + Unpin> {
    socket: &'a mut T,
    /// Bytes remaining before the current fragment is full.
    remaining_in_fragment: usize,
    /// Total bytes not yet written, across all remaining fragments.
    total_remaining: usize,
    /// Maximum data bytes per fragment.
    max_frag_size: usize,
}

impl<'a, T: AsyncWrite + Unpin> FragmentedWriter<'a, T> {
    /// Creates a writer for `total` bytes using [`MAX_FRAGMENT_SIZE`]-byte fragments.
    ///
    /// Writes the first fragment header to the socket before returning.
    async fn new(socket: &'a mut T, total: usize) -> io::Result<Self> {
        Self::with_max_frag_size(socket, total, MAX_FRAGMENT_SIZE).await
    }

    /// Creates a writer for `total` bytes with a caller-supplied `max_frag_size`.
    ///
    /// Writes the first fragment header to the socket before returning.
    /// Primarily intended for testing with small fragment sizes.
    async fn with_max_frag_size(
        socket: &'a mut T,
        total: usize,
        max_frag_size: usize,
    ) -> io::Result<Self> {
        let first_frag_size = total.min(max_frag_size);
        socket
            .write_all(&Self::make_header(first_frag_size, first_frag_size == total).to_be_bytes())
            .await?;
        Ok(Self {
            socket,
            remaining_in_fragment: first_frag_size,
            total_remaining: total,
            max_frag_size,
        })
    }

    /// Writes `data` bytes into the current (and subsequent) fragments.
    ///
    /// A new fragment header is emitted automatically each time the current fragment fills up.
    async fn write_bytes(&mut self, mut data: &[u8]) -> io::Result<()> {
        while !data.is_empty() {
            if self.remaining_in_fragment == 0 {
                // All declared bytes have been written; ignore surplus to avoid an infinite loop.
                break;
            }
            let to_write = data.len().min(self.remaining_in_fragment);
            self.socket.write_all(&data[..to_write]).await?;
            data = &data[to_write..];
            self.remaining_in_fragment -= to_write;
            self.total_remaining -= to_write;

            if self.remaining_in_fragment == 0 && self.total_remaining > 0 {
                let next_size = self.total_remaining.min(self.max_frag_size);
                let is_last = next_size == self.total_remaining;
                self.socket.write_all(&Self::make_header(next_size, is_last).to_be_bytes()).await?;
                self.remaining_in_fragment = next_size;
            }
        }
        Ok(())
    }

    /// Builds an RMS fragment header `u32`.
    ///
    /// Bit 31 ([`HEADER_MASK`]) is set when `is_last` is `true`; bits 30..0 hold `frag_size`.
    #[inline]
    fn make_header(frag_size: usize, is_last: bool) -> u32 {
        if is_last {
            (HEADER_MASK | frag_size) as u32
        } else {
            frag_size as u32
        }
    }
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use tokio::io::AsyncWrite;

    use super::{Buffer, FragmentedWriter, WriteBuffer, DEFAULT_SIZE};

    // --- Test helpers ---

    /// Async writer that records every byte written to it.
    struct Collector(Vec<u8>);

    impl AsyncWrite for Collector {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            self.0.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    /// Parses RMS-framed bytes into `(is_last, fragment_data)` pairs.
    fn parse_fragments(raw: &[u8]) -> Vec<(bool, Vec<u8>)> {
        let mut out = Vec::new();
        let mut pos = 0;
        while pos + 4 <= raw.len() {
            let hdr = u32::from_be_bytes(raw[pos..pos + 4].try_into().unwrap());
            let is_last = (hdr >> 31) == 1;
            let size = (hdr & 0x7FFF_FFFF) as usize;
            pos += 4;
            out.push((is_last, raw[pos..pos + size].to_vec()));
            pos += size;
            if is_last {
                break;
            }
        }
        out
    }

    /// A [`Buffer`] implementation backed by a `Vec<u8>`.
    struct VecBuffer(Vec<u8>);

    impl Buffer for VecBuffer {
        fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
            std::iter::once(self.0.as_slice())
        }

        fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
            std::iter::once(self.0.as_mut_slice())
        }

        fn len(&self) -> usize {
            self.0.len()
        }

        fn is_empty(&self) -> bool {
            self.0.is_empty()
        }

        fn empty() -> Self {
            VecBuffer(Vec::new())
        }
    }

    /// A zero-size [`Buffer`] with no backing data.
    struct NoopBuffer;

    impl Buffer for NoopBuffer {
        fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
            std::iter::empty()
        }

        fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
            std::iter::empty()
        }

        fn len(&self) -> usize {
            0
        }

        fn is_empty(&self) -> bool {
            true
        }

        fn empty() -> Self {
            NoopBuffer
        }
    }

    // --- FragmentedWriter unit tests ---

    #[tokio::test]
    async fn fragmented_writer_empty() {
        let mut col = Collector(vec![]);
        let _fw = FragmentedWriter::with_max_frag_size(&mut col, 0, 4).await.unwrap();
        let frags = parse_fragments(&col.0);
        assert_eq!(frags.len(), 1, "empty record must still emit one fragment");
        assert!(frags[0].0, "must be marked as the last fragment");
        assert!(frags[0].1.is_empty());
    }

    #[tokio::test]
    async fn fragmented_writer_single_fragment() {
        let data = [1u8, 2, 3];
        let mut col = Collector(vec![]);
        let mut fw = FragmentedWriter::with_max_frag_size(&mut col, data.len(), 8).await.unwrap();
        fw.write_bytes(&data).await.unwrap();
        let frags = parse_fragments(&col.0);
        assert_eq!(frags.len(), 1);
        assert!(frags[0].0, "single fragment must be marked last");
        assert_eq!(frags[0].1, data);
    }

    #[tokio::test]
    async fn fragmented_writer_exact_fragment_boundary() {
        // total == max_frag_size → exactly one fragment
        let data = [0u8; 4];
        let mut col = Collector(vec![]);
        let mut fw = FragmentedWriter::with_max_frag_size(&mut col, 4, 4).await.unwrap();
        fw.write_bytes(&data).await.unwrap();
        let frags = parse_fragments(&col.0);
        assert_eq!(frags.len(), 1);
        assert!(frags[0].0);
        assert_eq!(frags[0].1.len(), 4);
    }

    #[tokio::test]
    async fn fragmented_writer_two_fragments() {
        // 8 bytes, max 4 per fragment → [1..4] | [5..8]
        let data: Vec<u8> = (1u8..=8).collect();
        let mut col = Collector(vec![]);
        let mut fw = FragmentedWriter::with_max_frag_size(&mut col, data.len(), 4).await.unwrap();
        fw.write_bytes(&data).await.unwrap();
        let frags = parse_fragments(&col.0);
        assert_eq!(frags.len(), 2);
        assert!(!frags[0].0, "first fragment must not be marked last");
        assert_eq!(frags[0].1, &[1, 2, 3, 4]);
        assert!(frags[1].0, "second fragment must be marked last");
        assert_eq!(frags[1].1, &[5, 6, 7, 8]);
    }

    #[tokio::test]
    async fn fragmented_writer_three_fragments_partial_last() {
        // 10 bytes, max 4 per fragment → [1..4] | [5..8] | [9..10]
        let data: Vec<u8> = (1u8..=10).collect();
        let mut col = Collector(vec![]);
        let mut fw = FragmentedWriter::with_max_frag_size(&mut col, data.len(), 4).await.unwrap();
        fw.write_bytes(&data).await.unwrap();
        let frags = parse_fragments(&col.0);
        assert_eq!(frags.len(), 3);
        assert!(!frags[0].0);
        assert_eq!(frags[0].1, &[1, 2, 3, 4]);
        assert!(!frags[1].0);
        assert_eq!(frags[1].1, &[5, 6, 7, 8]);
        assert!(frags[2].0);
        assert_eq!(frags[2].1, &[9, 10]);
    }

    #[tokio::test]
    async fn fragmented_writer_multiple_write_calls_crossing_boundary() {
        // Writes that straddle fragment boundaries must produce correct byte ordering
        let mut col = Collector(vec![]);
        let mut fw = FragmentedWriter::with_max_frag_size(&mut col, 10, 4).await.unwrap();
        fw.write_bytes(&[1, 2, 3]).await.unwrap(); //   3/4 of frag 0
        fw.write_bytes(&[4, 5, 6, 7]).await.unwrap(); // fills frag 0, 3/4 of frag 1
        fw.write_bytes(&[8, 9, 10]).await.unwrap(); //   fills frag 1, frag 2
        let frags = parse_fragments(&col.0);
        assert_eq!(frags.len(), 3);
        let all: Vec<u8> = frags.iter().flat_map(|(_, d)| d.iter().copied()).collect();
        assert_eq!(all, (1u8..=10).collect::<Vec<_>>());
        assert!(frags[2].0, "last fragment must be marked");
    }

    // --- WriteBuffer integration tests ---

    #[tokio::test]
    async fn write_buffer_send_inner_buffer_single_fragment() {
        let mut wb: WriteBuffer<NoopBuffer, _> = WriteBuffer::new(Collector(vec![]), DEFAULT_SIZE);
        wb.buf.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        wb.send_inner_buffer().await.unwrap();
        let frags = parse_fragments(&wb.socket.0);
        assert_eq!(frags.len(), 1);
        assert!(frags[0].0);
        assert_eq!(frags[0].1, &[0xAA, 0xBB, 0xCC]);
        assert!(wb.buf.is_empty(), "staging buffer must be cleared after send");
    }

    #[tokio::test]
    async fn write_buffer_send_inner_buffer_empty() {
        let mut wb: WriteBuffer<NoopBuffer, _> = WriteBuffer::new(Collector(vec![]), DEFAULT_SIZE);
        wb.send_inner_buffer().await.unwrap();
        let frags = parse_fragments(&wb.socket.0);
        assert_eq!(frags.len(), 1);
        assert!(frags[0].0);
        assert!(frags[0].1.is_empty());
    }

    #[tokio::test]
    async fn write_buffer_send_inner_with_buffer_single_fragment() {
        // Staging buffer: 4-byte XDR header
        // Payload: 8 bytes (count 8, no padding needed since 8 % 4 == 0)
        let payload: Vec<u8> = (1u8..=8).collect();
        let count = payload.len();

        let mut wb: WriteBuffer<VecBuffer, _> = WriteBuffer::new(Collector(vec![]), DEFAULT_SIZE);
        wb.buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // dummy XDR header (4 bytes)
        wb.send_inner_with_buffer(VecBuffer(payload.clone()), count).await.unwrap();

        let frags = parse_fragments(&wb.socket.0);
        assert_eq!(frags.len(), 1);
        assert!(frags[0].0);
        // Wire layout: [XDR header 4B] + [count u32 4B] + [payload 8B] = 16 bytes
        assert_eq!(frags[0].1.len(), 4 + 4 + 8);
        // The last 8 bytes of the fragment must be the payload
        assert_eq!(&frags[0].1[8..], payload.as_slice());
        assert!(wb.buf.is_empty(), "staging buffer must be cleared after send");
    }

    #[tokio::test]
    async fn write_buffer_send_inner_with_buffer_padding() {
        // Payload of 3 bytes requires 1 byte of XDR padding
        let payload = vec![0xAA, 0xBB, 0xCC];
        let count = payload.len(); // 3

        let mut wb: WriteBuffer<VecBuffer, _> = WriteBuffer::new(Collector(vec![]), DEFAULT_SIZE);
        wb.send_inner_with_buffer(VecBuffer(payload.clone()), count).await.unwrap();

        let frags = parse_fragments(&wb.socket.0);
        assert_eq!(frags.len(), 1);
        assert!(frags[0].0);
        // Wire layout: [count u32 4B] + [payload 3B] + [padding 1B] = 8 bytes
        assert_eq!(frags[0].1.len(), 4 + 3 + 1);
        // count field = 3 (big-endian)
        assert_eq!(&frags[0].1[..4], &[0, 0, 0, 3]);
        // payload bytes
        assert_eq!(&frags[0].1[4..7], &[0xAA, 0xBB, 0xCC]);
        // padding byte must be zero
        assert_eq!(frags[0].1[7], 0x00);
    }
}
