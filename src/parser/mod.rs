//! Defines NFSv3 and MOUNT protocol parsing functionality.

pub mod mount;
pub mod nfsv3;
mod parser_struct;
pub mod primitive;
mod read_buffer;
mod rpc;

#[cfg(test)]
mod tests;

use std::future::Future;

use crate::mount::mnt::MountArgs;
use crate::mount::umnt::UnmountArgs;
use crate::rpc::Error;
use crate::vfs::{
    access, commit, create, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node, path_conf,
    read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink, write,
};

/// Result of parsing operations with errors type [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Helper function to process nested errors.
/// Function takes `future` to call. If result is `OK`, discards it, and returns `error`.
/// If `future` returns error - returns new one, rather than `error`
pub async fn proc_nested_errors<T>(error: Error, future: impl Future<Output = Result<T>>) -> Error {
    match future.await {
        Ok(_) => error,
        Err(err) => err,
    }
}

/// Enumerates the different types of arguments that can be parsed.
#[allow(dead_code)]
pub enum Arguments {
    // NFSv3
    /// Null operation arguments.
    Null,
    /// Arguments for the [`get_attr`] operation.
    GetAttr(get_attr::Args),
    /// Arguments for the [`set_attr`] operation.
    SetAttr(set_attr::Args),
    /// Arguments for the [`lookup`] operation.
    LookUp(lookup::Args),
    /// Arguments for the [`access`] operation.
    Access(access::Args),
    /// Arguments for the [`read_link`] operation.
    ReadLink(read_link::Args),
    /// Arguments for the [`read`] operation.
    Read(read::Args),
    /// Arguments for the [`mod@write`] operation.
    Write(write::Args),
    /// Arguments for the [`create`] operation.
    Create(create::Args),
    /// Arguments for the [`mk_dir`] operation.
    MkDir(mk_dir::Args),
    /// Arguments for the [`symlink`] operation.
    SymLink(symlink::Args),
    /// Arguments for the [`mk_node`] operation.
    MkNod(mk_node::Args),
    /// Arguments for the [`remove`] operation.
    Remove(remove::Args),
    /// Arguments for the [`rm_dir`] operation.
    RmDir(rm_dir::Args),
    /// Arguments for the [`rename`] operation.
    Rename(rename::Args),
    /// Arguments for the [`link`] operation.
    Link(link::Args),
    /// Arguments for the [`read_dir`] operation.
    ReadDir(read_dir::Args),
    /// Arguments for the [`read_dir_plus`] operation.
    ReadDirPlus(read_dir_plus::Args),
    /// Arguments for the [`fs_stat`] operation.
    FsStat(fs_stat::Args),
    /// Arguments for the [`fs_info`] operation.
    FsInfo(fs_info::Args),
    /// Arguments for the [`path_conf`] operation.
    PathConf(path_conf::Args),
    /// Arguments for the [`commit`] operation.
    Commit(commit::Args),
    // MOUNT
    /// Arguments for the Mount operation.
    Mount(MountArgs),
    /// Arguments for the Unmount operation.
    Unmount(UnmountArgs),
    /// Arguments for the Export operation.
    Export,
    /// Arguments for the Dump operation.
    Dump,
    /// Arguments for the UnmountAll operation.
    UnmountAll,
}
