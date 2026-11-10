use crate::{consts::nfsv3::NfsProc, parser::NfsArguments, vfs::NfsRes, Buffer};

impl<B: Buffer> NfsArguments<B> {
    /// Returns the NFSv3 procedure number of the call.
    pub(crate) fn procedure(&self) -> NfsProc {
        match self {
            Self::Null => NfsProc::Null,
            Self::GetAttr(_) => NfsProc::GetAttr,
            Self::SetAttr(_) => NfsProc::SetAttr,
            Self::LookUp(_) => NfsProc::LookUp,
            Self::Access(_) => NfsProc::Access,
            Self::ReadLink(_) => NfsProc::ReadLink,
            Self::Read(..) => NfsProc::Read,
            Self::Write(_) => NfsProc::Write,
            Self::Create(_) => NfsProc::Create,
            Self::MkDir(_) => NfsProc::MkDir,
            Self::SymLink(_) => NfsProc::SymLink,
            Self::MkNod(_) => NfsProc::MkNod,
            Self::Remove(_) => NfsProc::Remove,
            Self::RmDir(_) => NfsProc::RmDir,
            Self::Rename(_) => NfsProc::Rename,
            Self::Link(_) => NfsProc::Link,
            Self::ReadDir(_) => NfsProc::ReadDir,
            Self::ReadDirPlus(_) => NfsProc::ReadDirPlus,
            Self::FsStat(_) => NfsProc::FsStat,
            Self::FsInfo(_) => NfsProc::FsInfo,
            Self::PathConf(_) => NfsProc::PathConf,
            Self::Commit(_) => NfsProc::Commit,
        }
    }
}

impl<B: Buffer> NfsRes<B> {
    /// Returns the NFSv3 procedure number this is a result of.
    pub(crate) fn procedure(&self) -> NfsProc {
        match self {
            NfsRes::Null => NfsProc::Null,
            NfsRes::GetAttr(_) => NfsProc::GetAttr,
            NfsRes::SetAttr(_) => NfsProc::SetAttr,
            NfsRes::LookUp(_) => NfsProc::LookUp,
            NfsRes::Access(_) => NfsProc::Access,
            NfsRes::ReadLink(_) => NfsProc::ReadLink,
            NfsRes::Read(_) => NfsProc::Read,
            NfsRes::Write(_) => NfsProc::Write,
            NfsRes::Create(_) => NfsProc::Create,
            NfsRes::MkDir(_) => NfsProc::MkDir,
            NfsRes::SymLink(_) => NfsProc::SymLink,
            NfsRes::MkNod(_) => NfsProc::MkNod,
            NfsRes::Remove(_) => NfsProc::Remove,
            NfsRes::RmDir(_) => NfsProc::RmDir,
            NfsRes::Rename(_) => NfsProc::Rename,
            NfsRes::Link(_) => NfsProc::Link,
            NfsRes::ReadDir(_) => NfsProc::ReadDir,
            NfsRes::ReadDirPlus(_) => NfsProc::ReadDirPlus,
            NfsRes::FsStat(_) => NfsProc::FsStat,
            NfsRes::FsInfo(_) => NfsProc::FsInfo,
            NfsRes::PathConf(_) => NfsProc::PathConf,
            NfsRes::Commit(_) => NfsProc::Commit,
        }
    }
}
