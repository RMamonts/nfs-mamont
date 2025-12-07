//! Defines callback interface for [`crate::vfs::Vfs`]'s operations result processing.

use std::path::PathBuf;

use crate::vfs;

pub trait GetAttr {
    fn keep(self, promise: vfs::Result<vfs::file::Attr>);
}

pub trait SetAttr {
    fn keep(self, promise: vfs::Result<vfs::WccData>);
}

pub trait Lookup {
    fn keep(self, promise: vfs::Result<vfs::LookupResult>);
}

pub trait Access {
    fn keep(self, promise: vfs::Result<vfs::AccessResult>);
}

pub trait ReadLink {
    fn keep(self, promise: vfs::Result<(PathBuf, Option<vfs::file::Attr>)>);
}

pub trait Read {
    fn keep(self, promise: vfs::Result<vfs::ReadResult>);
}

pub trait Write {
    fn keep(self, promise: vfs::Result<vfs::WriteResult>);
}

pub trait Create {
    fn keep(self, promise: vfs::Result<vfs::CreatedNode>);
}

pub trait MakeDir {
    fn keep(self, promise: vfs::Result<vfs::CreatedNode>);
}

pub trait MakeSymlink {
    fn keep(self, promise: vfs::Result<vfs::CreatedNode>);
}

pub trait MakeNode {
    fn keep(self, promise: vfs::Result<vfs::CreatedNode>);
}

pub trait Remove {
    fn keep(self, promise: vfs::Result<vfs::RemovalResult>);
}

pub trait RemoveDir {
    fn keep(self, promise: vfs::Result<vfs::RemovalResult>);
}

pub trait Rename {
    fn keep(self, promise: vfs::Result<vfs::RenameResult>);
}

pub trait Link {
    fn keep(self, promise: vfs::Result<vfs::LinkResult>);
}

pub trait ReadDir {
    fn keep(self, promise: vfs::Result<vfs::ReadDirResult>);
}

pub trait ReadDirPlus {
    fn keep(self, promise: vfs::Result<vfs::ReadDirPlusResult>);
}

pub trait FsStat {
    fn keep(self, promise: vfs::Result<vfs::FsStat>);
}

pub trait FsInfo {
    fn keep(self, promise: vfs::Result<vfs::FsInfo>);
}

pub trait PathConf {
    fn keep(self, promise: vfs::Result<vfs::PathConfig>);
}

pub trait Commit {
    fn keep(self, promise: vfs::Result<vfs::CommitResult>);
}
