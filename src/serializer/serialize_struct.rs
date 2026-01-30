use crate::serializer::mount::{MountBody, MountStat};
use crate::serializer::nfs::results::{
    AccessResFail, AccessResOk, CommitResFail, CommitResOk, CreateResFail, CreateResOk,
    FsinfoResFail, FsinfoResOk, FsstatResFail, FsstatResOk, GetAttrResOk, LinkResFail, LinkResOk,
    LookUpResFail, LookUpResOk, MkdirResFail, MkdirResOk, MknodResOk, PathConfResFail,
    PathConfResOk, ReadDirPlusResFail, ReadDirPlusResOk, ReadDirResFail, ReadDirResOk,
    ReadLinkResFail, ReadLinkResOk, ReadResFail, ReadResOk, RemoveResFail, RemoveResOk,
    RenameResFail, RenameResOk, RmdirResFail, RmdirResOk, SetAttrResFail, SetAttrResOk,
    SymlinkResFail, SymlinkResOk, WriteResFail, WriteResOk,
};
use crate::vfs::NfsError;

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
    FsStat(Result<FsstatResOk, FsstatResFail>),
    FsInfo(Result<FsinfoResOk, FsinfoResFail>),
    PathConf(Result<PathConfResOk, PathConfResFail>),
    Commit(Result<CommitResOk, CommitResFail>),
}

#[allow(dead_code)]
pub enum MountRes {
    Mount(Result<MknodResOk, ()>),
    Unmount,
    Export,
    Dump(MountBody),
    UnmountAll,
}

#[allow(dead_code)]
pub enum ProcResult {
    Nfs3 { status: NfsError, data: NfsRes },
    Mount { status: MountStat, data: MountRes },
}
