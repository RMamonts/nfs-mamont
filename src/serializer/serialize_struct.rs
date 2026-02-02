use std::cmp::min;
use std::io;
use std::io::Write;

use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::allocator::Slice;
use crate::rpc::{AcceptStat, Error, OpaqueAuth, RejectedReply, ReplyBody, RpcBody};
use crate::serializer::mount::{
    export_node, mount_body, mount_res_ok, mount_stat, ExportNode, MountBody, MountResOk, MountStat,
};

use crate::serializer::nfs::access::{access_res_fail, access_res_ok};
use crate::serializer::nfs::commit::{commit_res_fail, commit_res_ok};
use crate::serializer::nfs::create::{create_res_fail, create_res_ok};
use crate::serializer::nfs::error;
use crate::serializer::nfs::fs_info::{fs_info_res_fail, fs_info_res_ok};
use crate::serializer::nfs::fs_stat::{fs_stat_res_fail, fs_stat_res_ok};
use crate::serializer::nfs::get_attr::get_attr_res_ok;
use crate::serializer::nfs::link::{link_res_fail, link_res_ok};
use crate::serializer::nfs::lookup::{lookup_res_fail, lookup_res_ok};
use crate::serializer::nfs::mk_dir::{mkdir_res_fail, mkdir_res_ok};
use crate::serializer::nfs::mk_node::{mknod_res_fail, mknod_res_ok};
use crate::serializer::nfs::path_conf::{path_config_res_fail, path_config_res_ok};
use crate::serializer::nfs::read::{read_res_fail, read_res_ok_partial};
use crate::serializer::nfs::read_dir::{read_dir_res_fail, read_dir_res_ok};
use crate::serializer::nfs::read_dir_plus::{read_dir_plus_res_fail, read_dir_plus_res_ok};
use crate::serializer::nfs::read_link::{read_link_res_fail, read_link_res_ok};
use crate::serializer::nfs::remove::{remove_res_fail, remove_res_ok};
use crate::serializer::nfs::rename::{rename_res_fail, rename_res_ok};
use crate::serializer::nfs::rm_dir::{rmdir_res_fail, rmdir_res_ok};
use crate::serializer::nfs::set_attr::{set_attr_res_fail, set_attr_res_ok};
use crate::serializer::nfs::symlink::{symlink_res_fail, symlink_res_ok};
use crate::serializer::nfs::write::{write_res_fail, write_res_ok};
use crate::serializer::rpc::auth_opaque;
use crate::serializer::{option, u32, usize_as_u32};
use crate::vfs::{
    access, commit, create, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node, path_conf,
    read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink, write,
    STATUS_OK,
};

#[allow(dead_code)]
// check actual max size
const DEFAUT_SIZE: usize = 4096;

#[allow(dead_code)]
pub enum NfsRes {
    Null,
    GetAttr(get_attr::Result),
    SetAttr(set_attr::Result),
    LookUp(lookup::Result),
    Access(access::Result),
    ReadLink(read_link::Result),
    Read(read::Result),
    Write(write::Result),
    Create(create::Result),
    MkDir(mk_dir::Result),
    SymLink(symlink::Result),
    MkNod(mk_node::Result),
    Remove(remove::Result),
    RmDir(rm_dir::Result),
    Rename(rename::Result),
    Link(link::Result),
    ReadDir(read_dir::Result),
    ReadDirPlus(read_dir_plus::Result),
    FsStat(fs_stat::Result),
    FsInfo(fs_info::Result),
    PathConf(path_conf::Result),
    Commit(commit::Result),
}

#[allow(dead_code)]
pub enum MountRes {
    Null,
    Mount(Result<MountResOk, ()>),
    Unmount,
    Export(Option<ExportNode>),
    Dump(Option<MountBody>),
    UnmountAll,
}

#[allow(dead_code)]
pub enum ProcResult {
    Nfs3(NfsRes),
    Mount { status: MountStat, data: MountRes },
}

pub struct ReplyFromVfs {
    xid: u32,
    verf: OpaqueAuth,
    // maybe move this Error from parser?
    proc_result: Result<ProcResult, Error>,
}

// maybe split underlying buffer?
#[allow(dead_code)]
pub struct Serializer<T: AsyncWrite + Unpin> {
    buffer: WriteBuffer<T>,
}

#[allow(dead_code)]
impl<T: AsyncWrite + Unpin> Serializer<T> {
    fn new(writer: T) -> Self {
        Self { buffer: WriteBuffer::new(writer, DEFAUT_SIZE) }
    }

    fn with_capacity(writer: T, capacity: usize) -> Self {
        Self { buffer: WriteBuffer::new(writer, capacity) }
    }

    async fn process_result(&mut self, result: ProcResult) -> io::Result<()> {
        match result {
            ProcResult::Nfs3(data) => match data {
                NfsRes::Null => self.buffer.send_inner_buffer().await,
                NfsRes::GetAttr(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        get_attr_res_ok(&mut self.buffer, ok)?;
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
                        set_attr_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        set_attr_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::LookUp(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        lookup_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        lookup_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Access(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        access_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        access_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::ReadLink(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        read_link_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        read_link_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Read(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        read_res_ok_partial(&mut self.buffer, ok.head)?;
                        self.buffer.send_inner_with_slice(ok.data).await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        read_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Write(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        write_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        write_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Create(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        create_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        create_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::MkDir(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        mkdir_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        mkdir_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::SymLink(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        symlink_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        symlink_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::MkNod(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        mknod_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        mknod_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Remove(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        remove_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        remove_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::RmDir(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        rmdir_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        rmdir_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Rename(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        rename_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        rename_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Link(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        link_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        link_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::ReadDir(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        read_dir_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        read_dir_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::ReadDirPlus(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        read_dir_plus_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        read_dir_plus_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::FsStat(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        fs_stat_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        fs_stat_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::FsInfo(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        fs_info_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        fs_info_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::PathConf(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        path_config_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        path_config_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Commit(res) => match res {
                    Ok(ok) => {
                        usize_as_u32(&mut self.buffer, STATUS_OK)?;
                        commit_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        error(&mut self.buffer, err.status)?;
                        commit_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
            },
            ProcResult::Mount { status, data } => match data {
                MountRes::Null | MountRes::UnmountAll | MountRes::Unmount => Ok(()),
                MountRes::Mount(res) => match res {
                    Ok(ok) => {
                        mount_stat(&mut self.buffer, status)?;
                        mount_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(_) => {
                        mount_stat(&mut self.buffer, status)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                MountRes::Export(node) => {
                    option(&mut self.buffer, node, |arg, dest| export_node(dest, arg))?;
                    self.buffer.send_inner_buffer().await
                }
                MountRes::Dump(body) => {
                    option(&mut self.buffer, body, |arg, dest| mount_body(dest, arg))?;
                    self.buffer.send_inner_buffer().await
                }
            },
        }
    }

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

struct WriteBuffer<T: AsyncWrite + Unpin> {
    socket: T,
    buf: Vec<u8>,
    position: usize,
}

impl<T: AsyncWrite + Unpin> Write for WriteBuffer<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let size = min(self.buf.len() + buf.len(), self.buf.len());
        self.buf[self.position..self.position + size].copy_from_slice(&buf[..size]);
        self.position += size;
        Ok(size)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<T: AsyncWrite + Unpin> WriteBuffer<T> {
    fn new(socket: T, capacity: usize) -> WriteBuffer<T> {
        WriteBuffer { socket, buf: vec![0u8; capacity], position: 0 }
    }

    fn clean(&mut self) {
        self.position = 0;
    }

    async fn send_inner_buffer(&mut self) -> io::Result<()> {
        self.socket.write_all(&self.buf[0..self.position]).await?;
        self.clean();
        Ok(())
    }

    async fn send_inner_with_slice(&mut self, slice: Slice) -> io::Result<()> {
        self.socket.write_all(&self.buf[0..self.position]).await?;
        // later change to explicit cursor (when one implemented)
        for buf in slice.iter() {
            self.socket.write_all(buf).await?;
        }
        self.clean();
        Ok(())
    }
}
