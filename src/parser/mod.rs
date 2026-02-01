use crate::rpc::Error;

use std::future::Future;
use std::io;
use std::string::FromUtf8Error;

use crate::parser::mount::{MountArgs, UnmountArgs};
use crate::parser::rpc::AuthStat;
use crate::vfs::{
    access, commit, create, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node, path_conf,
    read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink, write,
};

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
pub enum Arguments {
    // NFSv3
    Null,
    GetAttr(get_attr::Args),
    SetAttr(set_attr::Args),
    LookUp(lookup::Args),
    Access(access::Args),
    ReadLink(read_link::Args),
    Read(read::Args),
    Write(write::Args),
    Create(create::Args),
    MkDir(mk_dir::Args),
    SymLink(symlink::Args),
    MkNod(mk_node::Args),
    Remove(remove::Args),
    RmDir(rm_dir::Args),
    Rename(rename::Args),
    Link(link::Args),
    ReadDir(read_dir::Args),
    ReadDirPlus(read_dir_plus::Args),
    FsStat(fs_stat::Args),
    FsInfo(fs_info::Args),
    PathConf(path_conf::Args),
    Commit(commit::Args),
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
>>>>>>> svmk17/alt_parser
