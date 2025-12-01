use crate::parser::mount::{MountArgs, UnmountArgs};
use crate::parser::nfsv3::procedures::{
    AccessArgs, CommitArgs, CreateArgs, FsInfoArgs, FsStatArgs, GetAttrArgs, LinkArgs, LookUpArgs,
    MkDirArgs, MkNodArgs, PathConfArgs, ReadArgs, ReadDirArgs, ReadDirPlusArgs, ReadLinkArgs,
    RemoveArgs, RenameArgs, RmDirArgs, SetAttrArgs, SymLinkArgs, WriteArgs,
};
use crate::parser::rpc::AuthStat;
use std::future::Future;
use std::io;
use std::string::FromUtf8Error;

pub mod mount;
pub mod nfsv3;
mod parser_struct;
pub mod primitive;
mod read_buffer;
mod rpc;
#[cfg(test)]
mod tests;

pub type Result<T> = std::result::Result<T, Error>;

pub async fn proc_nested_errors<T>(error: Error, fun: impl Future<Output = Result<T>>) -> Error {
    match fun.await {
        Ok(_) => error,
        Err(err) => err,
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ProgramVersionMismatch(u32, u32);
#[allow(dead_code)]
#[derive(Debug)]
pub struct RPCVersionMismatch(u32, u32);

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

#[derive(Debug)]
#[allow(unused)]
pub enum Error {
    MaxELemLimit,
    IO(io::Error),
    EnumDiscMismatch,
    IncorrectString(FromUtf8Error),
    IncorrectPadding,
    ImpossibleTypeCast,
    BadFileHandle,
    MessageTypeMismatch,
    RpcVersionMismatch(RPCVersionMismatch),
    AuthError(AuthStat),
    ProgramMismatch,
    ProcedureMismatch,
    ProgramVersionMismatch(ProgramVersionMismatch),
}
