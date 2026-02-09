//! High-level XDR serializer for complete RPC/NFS replies.
//!
//! This module bridges `crate::vfs` results to the wire format by selecting the
//! appropriate per-procedure serializer from `crate::serializer::nfs` (and
//! mount serializers from `crate::serializer::mount`), then emitting a complete
//! RPC reply to an async writer.

use std::cmp::min;
use std::io;
use std::io::Write;

use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::allocator::Slice;
use crate::mount::{dump, export};
use crate::rpc::{AcceptStat, Error, OpaqueAuth, RejectedReply, ReplyBody, RpcBody};
use crate::serializer::mount::mnt;
use crate::serializer::nfs::{
    access, commit, create, error, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node,
    path_conf, read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink,
    write,
};
use crate::serializer::rpc::auth_opaque;
use crate::serializer::{u32, usize_as_u32, ALIGNMENT};
use crate::vfs::STATUS_OK;
use crate::{mount, serializer, vfs};

// check actual max size
const DEFAULT_SIZE: usize = 4096;

/// Wrapper for all supported NFSv3 procedure result types coming from [`vfs`].
#[allow(unused)]
pub enum NfsRes {
    Null,
    GetAttr(vfs::get_attr::Result),
    SetAttr(vfs::set_attr::Result),
    LookUp(vfs::lookup::Result),
    Access(vfs::access::Result),
    ReadLink(vfs::read_link::Result),
    Read(vfs::read::Result),
    Write(vfs::write::Result),
    Create(vfs::create::Result),
    MkDir(vfs::mk_dir::Result),
    SymLink(vfs::symlink::Result),
    MkNod(vfs::mk_node::Result),
    Remove(vfs::remove::Result),
    RmDir(vfs::rm_dir::Result),
    Rename(vfs::rename::Result),
    Link(vfs::link::Result),
    ReadDir(vfs::read_dir::Result),
    ReadDirPlus(vfs::read_dir_plus::Result),
    FsStat(vfs::fs_stat::Result),
    FsInfo(vfs::fs_info::Result),
    PathConf(vfs::path_conf::Result),
    Commit(vfs::commit::Result),
}

/// Wrapper for mount procedure result bodies.
#[allow(unused)]
pub enum MountRes {
    Null,
    Mount(mount::mnt::Result),
    Unmount,
    Export(export::Success),
    Dump(dump::Success),
    UnmountAll,
}

/// Tagged union of top-level RPC program results supported by this server.
#[allow(dead_code, clippy::large_enum_variant)]
pub enum ProcResult {
    Nfs3(NfsRes),
    Mount(MountRes),
}

/// RPC reply metadata plus a typed result to be serialized.
pub struct ReplyFromVfs {
    xid: u32,
    verf: OpaqueAuth,
    proc_result: Result<ProcResult, Error>,
}

/// Async writer wrapper used to emit XDR-encoded RPC replies.
pub struct Serializer<T: AsyncWrite + Unpin> {
    buffer: WriteBuffer<T>,
}

#[allow(dead_code)]
impl<T: AsyncWrite + Unpin> Serializer<T> {
    /// Creates a reply serializer writing XDR bytes to the provided async writer.
    fn new(writer: T) -> Self {
        Self { buffer: WriteBuffer::new(writer, DEFAULT_SIZE) }
    }

    /// Creates a reply serializer with an explicit internal buffer capacity.
    fn with_capacity(writer: T, capacity: usize) -> Self {
        Self { buffer: WriteBuffer::new(writer, capacity) }
    }

    /// Serializes a [`ProcResult`] into its XDR reply body and writes it to the underlying writer.
    async fn process_result(&mut self, result: ProcResult) -> io::Result<()> {
        match result {
            ProcResult::Nfs3(data) => match data {
                NfsRes::Null => self.buffer.send_inner_buffer().await,
                NfsRes::GetAttr(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        get_attr::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::SetAttr(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        set_attr::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        set_attr::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::LookUp(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        lookup::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        lookup::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Access(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        access::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        access::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::ReadLink(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        read_link::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        read_link::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Read(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        read::result_ok_part(&mut self.buffer, ok.head)?;
                        self.buffer.send_inner_with_slice(ok.data).await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        read::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Write(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        write::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        write::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Create(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        create::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        create::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::MkDir(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        mk_dir::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        mk_dir::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::SymLink(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        symlink::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        symlink::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::MkNod(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        mk_node::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        mk_node::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Remove(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        remove::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        remove::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::RmDir(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        rm_dir::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        rm_dir::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Rename(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        rename::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        rename::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Link(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        link::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        link::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::ReadDir(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        read_dir::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        read_dir::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::ReadDirPlus(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        read_dir_plus::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        read_dir_plus::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::FsStat(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        fs_stat::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        fs_stat::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::FsInfo(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        fs_info::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        fs_info::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::PathConf(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        path_conf::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        path_conf::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Commit(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        commit::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.error)?;
                        commit::result_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
            },
            ProcResult::Mount(data) => match data {
                MountRes::Null | MountRes::UnmountAll | MountRes::Unmount => Ok(()),
                MountRes::Mount(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        mnt::result_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(stat) => {
                        serializer::mount::mount_stat(&mut self.buffer, stat)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                MountRes::Export(node) => {
                    serializer::mount::export::result_ok(&mut self.buffer, node)?;
                    self.buffer.send_inner_buffer().await
                }
                MountRes::Dump(body) => {
                    serializer::mount::dump::result_ok(&mut self.buffer, body)?;
                    self.buffer.send_inner_buffer().await
                }
            },
        }
    }

    /// Serializes [`ReplyFromVfs`] into a complete XDR RPC reply and writes it to the underlying writer.
    pub async fn form_reply(&mut self, reply: ReplyFromVfs) -> io::Result<()> {
        u32(&mut self.buffer, reply.xid)?;
        u32(&mut self.buffer, RpcBody::Reply as u32)?;
        match reply.proc_result {
            Ok(proc) => {
                u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                auth_opaque(&mut self.buffer, reply.verf)?;
                u32(&mut self.buffer, AcceptStat::Success as u32)?;
                self.process_result(proc).await
            }
            Err(err) => match err {
                Error::IncorrectPadding
                | Error::ImpossibleTypeCast
                | Error::BadFileHandle
                | Error::MessageTypeMismatch
                | Error::EnumDiscMismatch
                | Error::MaxELemLimit
                | Error::IncorrectString(_) => {
                    u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                    auth_opaque(&mut self.buffer, reply.verf)?;
                    // or maybe system error?
                    u32(&mut self.buffer, AcceptStat::GarbageArgs as u32)
                }
                Error::RpcVersionMismatch(vers) => {
                    u32(&mut self.buffer, ReplyBody::MsgDenied as u32)?;
                    auth_opaque(&mut self.buffer, reply.verf)?;
                    u32(&mut self.buffer, RejectedReply::RpcMismatch as u32)?;
                    u32(&mut self.buffer, vers.low)?;
                    u32(&mut self.buffer, vers.high)
                }
                Error::AuthError(stat) => {
                    u32(&mut self.buffer, ReplyBody::MsgDenied as u32)?;
                    auth_opaque(&mut self.buffer, reply.verf)?;
                    u32(&mut self.buffer, RejectedReply::AuthError as u32)?;
                    u32(&mut self.buffer, stat as u32)
                }
                Error::ProgramMismatch => {
                    u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                    auth_opaque(&mut self.buffer, reply.verf)?;
                    u32(&mut self.buffer, AcceptStat::ProgUnavail as u32)
                }
                Error::ProcedureMismatch => {
                    u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                    auth_opaque(&mut self.buffer, reply.verf)?;
                    u32(&mut self.buffer, AcceptStat::ProcUnavail as u32)
                }
                Error::ProgramVersionMismatch(info) => {
                    u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                    auth_opaque(&mut self.buffer, reply.verf)?;
                    u32(&mut self.buffer, AcceptStat::ProgMismatch as u32)?;
                    u32(&mut self.buffer, info.low)?;
                    u32(&mut self.buffer, info.high)
                }
                Error::IO(_) => {
                    u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                    auth_opaque(&mut self.buffer, reply.verf)?;
                    // or maybe system error?
                    u32(&mut self.buffer, AcceptStat::SystemErr as u32)
                }
            },
        }
    }
}

/// Buffered async writer used by the high-level reply serializer.
struct WriteBuffer<T: AsyncWrite + Unpin> {
    socket: T,
    buf: Vec<u8>,
    position: usize,
}

impl<T: AsyncWrite + Unpin> Write for WriteBuffer<T> {
    /// Writes raw bytes into the internal staging buffer (not directly to the socket).
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let size = min(self.buf.len() + buf.len(), self.buf.len());
        self.buf[self.position..self.position + size].copy_from_slice(&buf[..size]);
        self.position += size;
        Ok(size)
    }

    /// No-op flush (the buffer is flushed explicitly by `send_inner_*`).
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<T: AsyncWrite + Unpin> WriteBuffer<T> {
    /// Creates a new buffer around an async writer with a fixed preallocated capacity.
    fn new(socket: T, capacity: usize) -> WriteBuffer<T> {
        WriteBuffer { socket, buf: vec![0u8; capacity], position: 0 }
    }

    /// Resets the internal write cursor to the start of the buffer.
    fn clean(&mut self) {
        self.position = 0;
    }

    /// Flushes the staged XDR bytes to the underlying writer.
    async fn send_inner_buffer(&mut self) -> io::Result<()> {
        self.socket.write_all(&self.buf[0..self.position]).await?;
        self.clean();
        Ok(())
    }

    /// Flushes the staged XDR bytes followed by a streamed payload [`Slice`] (used for READ data).
    async fn send_inner_with_slice(&mut self, slice: Slice) -> io::Result<()> {
        self.socket.write_all(&self.buf[0..self.position]).await?;
        // later change to explicit cursor (when one implemented)
        for buf in slice.iter() {
            self.socket.write_all(buf).await?;
        }
        // write padding directly to socket
        let size = slice.iter().map(|b| b.len()).sum::<usize>();
        let padding = (ALIGNMENT - size % ALIGNMENT) % ALIGNMENT;
        let slice = [0u8; ALIGNMENT];
        self.socket.write_all(&slice[..padding]).await?;

        self.clean();
        Ok(())
    }
}
