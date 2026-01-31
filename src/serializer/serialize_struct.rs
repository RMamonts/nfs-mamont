use std::cmp::min;
use std::io;
use std::io::Write;

use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::allocator::Slice;
use crate::rpc::{AcceptStat, Error, OpaqueAuth, RejectedReply, ReplyBody, RpcBody};
use crate::serializer::mount::{
    export_node, mount_body, mount_stat, ExportNode, MountBody, MountStat,
};
use crate::serializer::nfs::results::{
    access_res_fail, access_res_ok, commit_res_fail, commit_res_ok, create_res_fail, create_res_ok,
    fs_info_res_fail, fs_info_res_ok, fs_stat_res_fail, fs_stat_res_ok, get_attr_res_ok,
    link_res_fail, link_res_ok, lookup_res_fail, lookup_res_ok, mkdir_res_fail, mkdir_res_ok,
    mknod_res_fail, mknod_res_ok, nfsstat3, path_config_res_fail, path_config_res_ok,
    read_dir_plus_res_fail, read_dir_plus_res_ok, read_dir_res_fail, read_dir_res_ok,
    read_link_res_fail, read_link_res_ok, read_res_fail, read_res_ok_partial, remove_res_fail,
    remove_res_ok, rename_res_fail, rename_res_ok, rmdir_res_fail, rmdir_res_ok, set_attr_res_fail,
    set_attr_res_ok, symlink_res_fail, symlink_res_ok, write_res_fail, write_res_ok, AccessResFail,
    AccessResOk, CommitResFail, CommitResOk, CreateResFail, CreateResOk, FsInfoResOk,
    FsStatResFail, FsStatResOk, FsinfoResFail, GetAttrResOk, LinkResFail, LinkResOk, LookUpResFail,
    LookUpResOk, MkdirResFail, MkdirResOk, MknodResFail, MknodResOk, PathConfResFail,
    PathConfResOk, ReadDirPlusResFail, ReadDirPlusResOk, ReadDirResFail, ReadDirResOk,
    ReadLinkResFail, ReadLinkResOk, ReadResFail, ReadResOk, RemoveResFail, RemoveResOk,
    RenameResFail, RenameResOk, RmdirResFail, RmdirResOk, SetAttrResFail, SetAttrResOk,
    SymlinkResFail, SymlinkResOk, WriteResFail, WriteResOk,
};
use crate::serializer::rpc::auth_opaque;
use crate::serializer::{option, u32};
use crate::vfs::NfsError;

#[allow(dead_code)]
// check actual max size
const DEFAUT_SIZE: usize = 4096;

#[allow(dead_code)]
pub enum NfsRes {
    Null,
    GetAttr(Result<GetAttrResOk, ()>),
    SetAttr(Result<SetAttrResOk, SetAttrResFail>),
    LookUp(Result<LookUpResOk, LookUpResFail>),
    Access(Result<AccessResOk, AccessResFail>),
    ReadLink(Result<ReadLinkResOk, ReadLinkResFail>),
    Read(Result<ReadResOk, ReadResFail>),
    Write(Result<WriteResOk, WriteResFail>),
    Create(Result<CreateResOk, CreateResFail>),
    MkDir(Result<MkdirResOk, MkdirResFail>),
    SymLink(Result<SymlinkResOk, SymlinkResFail>),
    MkNod(Result<MknodResOk, MknodResFail>),
    Remove(Result<RemoveResOk, RemoveResFail>),
    RmDir(Result<RmdirResOk, RmdirResFail>),
    Rename(Result<RenameResOk, RenameResFail>),
    Link(Result<LinkResOk, LinkResFail>),
    ReadDir(Result<ReadDirResOk, ReadDirResFail>),
    ReadDirPlus(Result<ReadDirPlusResOk, ReadDirPlusResFail>),
    FsStat(Result<FsStatResOk, FsStatResFail>),
    FsInfo(Result<FsInfoResOk, FsinfoResFail>),
    PathConf(Result<PathConfResOk, PathConfResFail>),
    Commit(Result<CommitResOk, CommitResFail>),
}

#[allow(dead_code)]
pub enum MountRes {
    Null,
    Mount(Result<MknodResOk, ()>),
    Unmount,
    Export(Option<ExportNode>),
    Dump(Option<MountBody>),
    UnmountAll,
}

#[allow(dead_code)]
pub enum ProcResult {
    Nfs3 { status: NfsError, data: NfsRes },
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
            ProcResult::Nfs3 { status, data } => match data {
                NfsRes::Null => self.buffer.send_inner_buffer().await,
                NfsRes::GetAttr(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        get_attr_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(_) => {
                        nfsstat3(&mut self.buffer, status)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::SetAttr(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        set_attr_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        set_attr_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::LookUp(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        lookup_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        lookup_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Access(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        access_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        access_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::ReadLink(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_link_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_link_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Read(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_res_ok_partial(&mut self.buffer, ok.head)?;
                        self.buffer.send_inner_with_slice(ok.data).await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Write(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        write_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        write_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Create(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        create_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        create_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::MkDir(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        mkdir_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        mkdir_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::SymLink(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        symlink_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        symlink_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::MkNod(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        mknod_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        mknod_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Remove(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        remove_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        remove_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::RmDir(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        rmdir_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        rmdir_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Rename(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        rename_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        rename_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Link(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        link_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        link_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::ReadDir(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_dir_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_dir_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::ReadDirPlus(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_dir_plus_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_dir_plus_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::FsStat(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        fs_stat_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        fs_stat_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::FsInfo(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        fs_info_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        fs_info_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::PathConf(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        path_config_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        path_config_res_fail(&mut self.buffer, err)?;
                        self.buffer.send_inner_buffer().await
                    }
                },
                NfsRes::Commit(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        commit_res_ok(&mut self.buffer, ok)?;
                        self.buffer.send_inner_buffer().await
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
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
                        mknod_res_ok(&mut self.buffer, ok)?;
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
