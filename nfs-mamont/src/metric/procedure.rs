use crate::{
    consts::nfsv3::NfsProc,
    parser::{ArgWrapper, NfsArguments, ProcArguments},
    rpc::ErrorWrapper,
    task::{ProcReply, ProcResult},
    vfs::NfsRes,
    Buffer,
};

/// Returns the NFSv3 procedure number of a parsed call, `None` for other programs and errors.
pub(crate) fn call_procedure<B: Buffer>(
    message: &Result<ArgWrapper<B>, ErrorWrapper>,
) -> Option<u32> {
    match message {
        Ok(ArgWrapper { proc: ProcArguments::Nfs3(args), .. }) => Some(args.procedure()),
        _ => None,
    }
}

/// Returns the NFSv3 procedure number of a reply, `None` for other programs and errors.
pub(crate) fn reply_procedure<B: Buffer>(reply: &ProcReply<B>) -> Option<u32> {
    match &reply.proc_result {
        Ok(ProcResult::Nfs3(res)) => Some(res.procedure()),
        _ => None,
    }
}

impl<B: Buffer> NfsArguments<B> {
    /// Returns the NFSv3 procedure number of the call.
    fn procedure(&self) -> u32 {
        match self {
            Self::Null => NfsProc::Null as u32,
            Self::GetAttr(_) => NfsProc::GetAttr as u32,
            Self::SetAttr(_) => NfsProc::SetAttr as u32,
            Self::LookUp(_) => NfsProc::LookUp as u32,
            Self::Access(_) => NfsProc::Access as u32,
            Self::ReadLink(_) => NfsProc::ReadLink as u32,
            Self::Read(..) => NfsProc::Read as u32,
            Self::Write(_) => NfsProc::Write as u32,
            Self::Create(_) => NfsProc::Create as u32,
            Self::MkDir(_) => NfsProc::MkDir as u32,
            Self::SymLink(_) => NfsProc::SymLink as u32,
            Self::MkNod(_) => NfsProc::MkNod as u32,
            Self::Remove(_) => NfsProc::Remove as u32,
            Self::RmDir(_) => NfsProc::RmDir as u32,
            Self::Rename(_) => NfsProc::Rename as u32,
            Self::Link(_) => NfsProc::Link as u32,
            Self::ReadDir(_) => NfsProc::ReadDir as u32,
            Self::ReadDirPlus(_) => NfsProc::ReadDirPlus as u32,
            Self::FsStat(_) => NfsProc::FsStat as u32,
            Self::FsInfo(_) => NfsProc::FsInfo as u32,
            Self::PathConf(_) => NfsProc::PathConf as u32,
            Self::Commit(_) => NfsProc::Commit as u32,
        }
    }
}

impl<B: Buffer> NfsRes<B> {
    /// Returns the NFSv3 procedure number this is a result of.
    fn procedure(&self) -> u32 {
        match self {
            NfsRes::Null => NfsProc::Null as u32,
            NfsRes::GetAttr(_) => NfsProc::GetAttr as u32,
            NfsRes::SetAttr(_) => NfsProc::SetAttr as u32,
            NfsRes::LookUp(_) => NfsProc::LookUp as u32,
            NfsRes::Access(_) => NfsProc::Access as u32,
            NfsRes::ReadLink(_) => NfsProc::ReadLink as u32,
            NfsRes::Read(_) => NfsProc::Read as u32,
            NfsRes::Write(_) => NfsProc::Write as u32,
            NfsRes::Create(_) => NfsProc::Create as u32,
            NfsRes::MkDir(_) => NfsProc::MkDir as u32,
            NfsRes::SymLink(_) => NfsProc::SymLink as u32,
            NfsRes::MkNod(_) => NfsProc::MkNod as u32,
            NfsRes::Remove(_) => NfsProc::Remove as u32,
            NfsRes::RmDir(_) => NfsProc::RmDir as u32,
            NfsRes::Rename(_) => NfsProc::Rename as u32,
            NfsRes::Link(_) => NfsProc::Link as u32,
            NfsRes::ReadDir(_) => NfsProc::ReadDir as u32,
            NfsRes::ReadDirPlus(_) => NfsProc::ReadDirPlus as u32,
            NfsRes::FsStat(_) => NfsProc::FsStat as u32,
            NfsRes::FsInfo(_) => NfsProc::FsInfo as u32,
            NfsRes::PathConf(_) => NfsProc::PathConf as u32,
            NfsRes::Commit(_) => NfsProc::Commit as u32,
        }
    }
}
