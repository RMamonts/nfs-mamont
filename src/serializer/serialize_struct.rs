use crate::serializer::mount::{
    export_node, mount_body, mount_stat, ExportNode, MountBody, MountStat,
};
use crate::serializer::nfs::results::{
    access_res_fail, access_res_ok, commit_res_fail, commit_res_ok, create_res_fail, create_res_ok,
    fs_info_res_fail, fs_info_res_ok, fs_stat_res_fail, fs_stat_res_ok, get_attr_res_ok,
    link_res_fail, link_res_ok, lookup_res_fail, lookup_res_ok, mkdir_res_fail, mkdir_res_ok,
    mknod_res_ok, nfsstat3, path_config_res_fail, path_config_res_ok, read_dir_plus_res_fail,
    read_dir_plus_res_ok, read_dir_res_fail, read_dir_res_ok, read_link_res_fail, read_link_res_ok,
    read_res_fail, read_res_ok_partial, remove_res_fail, remove_res_ok, rename_res_fail,
    rename_res_ok, rmdir_res_fail, rmdir_res_ok, set_attr_res_fail, set_attr_res_ok,
    symlink_res_fail, symlink_res_ok, write_res_fail, write_res_ok, AccessResFail, AccessResOk,
    CommitResFail, CommitResOk, CreateResFail, CreateResOk, FsInfoResOk, FsStatResFail,
    FsStatResOk, FsinfoResFail, GetAttrResOk, LinkResFail, LinkResOk, LookUpResFail, LookUpResOk,
    MkdirResFail, MkdirResOk, MknodResOk, PathConfResFail, PathConfResOk, ReadDirPlusResFail,
    ReadDirPlusResOk, ReadDirResFail, ReadDirResOk, ReadLinkResFail, ReadLinkResOk, ReadResFail,
    ReadResOk, RemoveResFail, RemoveResOk, RenameResFail, RenameResOk, RmdirResFail, RmdirResOk,
    SetAttrResFail, SetAttrResOk, SymlinkResFail, SymlinkResOk, WriteResFail, WriteResOk,
};
use crate::serializer::{option, u32};
use crate::vfs::NfsError;

use crate::rpc::{AcceptStat, Error, OpaqueAuth, RejectedReply, ReplyBody, RpcBody};
use crate::serializer::rpc::auth_opaque;
use std::io;
use std::io::Cursor;
use tokio::io::AsyncWrite;

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
    MkNod(Result<MknodResOk, MkdirResFail>),
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

struct ReplyFromVfs {
    xid: u32,
    verf: OpaqueAuth,
    // maybe move this Error from parser?
    proc_result: Result<ProcResult, Error>,
}

// maybe split underlying buffer?
#[allow(dead_code)]
pub struct Serializer<T: AsyncWrite + Unpin> {
    writer: T,
    buffer: Cursor<Vec<u8>>,
}

#[allow(dead_code)]
impl<T: AsyncWrite + Unpin> Serializer<T> {
    fn new(writer: T) -> Self {
        Self { writer, buffer: Cursor::new(vec![0u8; DEFAUT_SIZE]) }
    }

    fn with_capacity(writer: T, capacity: usize) -> Self {
        Self { writer, buffer: Cursor::new(Vec::with_capacity(capacity)) }
    }

    fn process_result(&mut self, result: ProcResult) -> io::Result<()> {
        match result {
            ProcResult::Nfs3 { status, data } => match data {
                NfsRes::Null => Ok(()),
                NfsRes::GetAttr(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        get_attr_res_ok(&mut self.buffer, ok)
                    }
                    Err(_) => nfsstat3(&mut self.buffer, status),
                },
                NfsRes::SetAttr(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        set_attr_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        set_attr_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::LookUp(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        lookup_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        lookup_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::Access(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        access_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        access_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::ReadLink(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_link_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_link_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::Read(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_res_ok_partial(&mut self.buffer, ok)
                        // need to think about Slice
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::Write(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        write_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        write_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::Create(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        create_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        create_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::MkDir(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        mkdir_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        mkdir_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::SymLink(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        symlink_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        symlink_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::MkNod(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        mknod_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        mkdir_res_fail(&mut self.buffer, err) // MkNod can return MkDirFail
                    }
                },
                NfsRes::Remove(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        remove_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        remove_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::RmDir(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        rmdir_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        rmdir_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::Rename(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        rename_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        rename_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::Link(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        link_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        link_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::ReadDir(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_dir_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_dir_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::ReadDirPlus(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_dir_plus_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        read_dir_plus_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::FsStat(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        fs_stat_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        fs_stat_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::FsInfo(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        fs_info_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        fs_info_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::PathConf(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        path_config_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        path_config_res_fail(&mut self.buffer, err)
                    }
                },
                NfsRes::Commit(res) => match res {
                    Ok(ok) => {
                        nfsstat3(&mut self.buffer, status)?;
                        commit_res_ok(&mut self.buffer, ok)
                    }
                    Err(err) => {
                        nfsstat3(&mut self.buffer, status)?;
                        commit_res_fail(&mut self.buffer, err)
                    }
                },
            },
            ProcResult::Mount { status, data } => match data {
                MountRes::Null | MountRes::UnmountAll | MountRes::Unmount => Ok(()),
                MountRes::Mount(res) => match res {
                    Ok(ok) => {
                        mount_stat(&mut self.buffer, status)?;
                        mknod_res_ok(&mut self.buffer, ok)
                    }
                    Err(_) => mount_stat(&mut self.buffer, status),
                },
                MountRes::Export(node) => {
                    option(&mut self.buffer, node, |arg, dest| export_node(dest, arg))
                }
                MountRes::Dump(body) => {
                    option(&mut self.buffer, body, |arg, dest| mount_body(dest, arg))
                }
            },
        }
    }

    fn form_reply(&mut self, reply: ReplyFromVfs) -> io::Result<()> {
        u32(&mut self.buffer, reply.xid)?;
        u32(&mut self.buffer, RpcBody::Reply as u32)?;
        match reply.proc_result {
            Ok(proc) => {
                u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                auth_opaque(&mut self.buffer, reply.verf)?;
                u32(&mut self.buffer, AcceptStat::Success as u32)?;
                self.process_result(proc)
            }
            Err(err) => match err {
                Error::IncorrectPadding
                | Error::ImpossibleTypeCast
                | Error::BadFileHandle
                | Error::MessageTypeMismatch => {
                    u32(&mut self.buffer, ReplyBody::MsgAccepted as u32)?;
                    auth_opaque(&mut self.buffer, reply.verf)?;
                    // or maybe system error?
                    u32(&mut self.buffer, AcceptStat::GarbageArgs as u32)
                }
                Error::RpcVersionMismatch(vers) => {
                    u32(&mut self.buffer, ReplyBody::MsgDenied as u32)?;
                    auth_opaque(&mut self.buffer, reply.verf)?;
                    u32(&mut self.buffer, RejectedReply::RPC_MISMATCH as u32)?;
                    u32(&mut self.buffer, vers.low)?;
                    u32(&mut self.buffer, vers.high)
                }
                Error::AuthError(stat) => {
                    u32(&mut self.buffer, ReplyBody::MsgDenied as u32)?;
                    auth_opaque(&mut self.buffer, reply.verf)?;
                    u32(&mut self.buffer, RejectedReply::AUTH_ERROR as u32)?;
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
            },
        }
    }
}
