use crate::{consts::nfsv3::*, parser::NfsArguments, vfs::NfsRes, Buffer};

impl<B: Buffer> NfsArguments<B> {
    /// Returns the NFSv3 procedure number of the call.
    pub(crate) fn procedure(&self) -> u32 {
        match self {
            Self::Null => NULL,
            Self::GetAttr(_) => GETATTR,
            Self::SetAttr(_) => SETATTR,
            Self::LookUp(_) => LOOKUP,
            Self::Access(_) => ACCESS,
            Self::ReadLink(_) => READLINK,
            Self::Read(..) => READ,
            Self::Write(_) => WRITE,
            Self::Create(_) => CREATE,
            Self::MkDir(_) => MKDIR,
            Self::SymLink(_) => SYMLINK,
            Self::MkNod(_) => MKNOD,
            Self::Remove(_) => REMOVE,
            Self::RmDir(_) => RMDIR,
            Self::Rename(_) => RENAME,
            Self::Link(_) => LINK,
            Self::ReadDir(_) => READDIR,
            Self::ReadDirPlus(_) => READDIRPLUS,
            Self::FsStat(_) => FSSTAT,
            Self::FsInfo(_) => FSINFO,
            Self::PathConf(_) => PATHCONF,
            Self::Commit(_) => COMMIT,
        }
    }
}

impl<B: Buffer> NfsRes<B> {
    /// Returns the NFSv3 procedure number this is a result of.
    pub(crate) fn procedure(&self) -> u32 {
        match self {
            NfsRes::Null => NULL,
            NfsRes::GetAttr(_) => GETATTR,
            NfsRes::SetAttr(_) => SETATTR,
            NfsRes::LookUp(_) => LOOKUP,
            NfsRes::Access(_) => ACCESS,
            NfsRes::ReadLink(_) => READLINK,
            NfsRes::Read(_) => READ,
            NfsRes::Write(_) => WRITE,
            NfsRes::Create(_) => CREATE,
            NfsRes::MkDir(_) => MKDIR,
            NfsRes::SymLink(_) => SYMLINK,
            NfsRes::MkNod(_) => MKNOD,
            NfsRes::Remove(_) => REMOVE,
            NfsRes::RmDir(_) => RMDIR,
            NfsRes::Rename(_) => RENAME,
            NfsRes::Link(_) => LINK,
            NfsRes::ReadDir(_) => READDIR,
            NfsRes::ReadDirPlus(_) => READDIRPLUS,
            NfsRes::FsStat(_) => FSSTAT,
            NfsRes::FsInfo(_) => FSINFO,
            NfsRes::PathConf(_) => PATHCONF,
            NfsRes::Commit(_) => COMMIT,
        }
    }
}
