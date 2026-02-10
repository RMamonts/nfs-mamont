use crate::parser::Arguments;

#[cfg(feature = "arbitrary")]
use crate::mount::MOUNT_PROGRAM;
#[cfg(feature = "arbitrary")]
use crate::nfsv3::NFS_PROGRAM;
#[cfg(feature = "arbitrary")]
use arbitrary::{Arbitrary, Unstructured};

const NULL: u32 = 0;

const GETATTR: u32 = 1;

const SETATTR: u32 = 2;

const LOOKUP: u32 = 3;

const ACCESS: u32 = 4;

const READLINK: u32 = 5;

const READ: u32 = 6;

const WRITE: u32 = 7;

const CREATE: u32 = 8;

const MKDIR: u32 = 9;

const SYMLINK: u32 = 10;

const MKNOD: u32 = 11;

const REMOVE: u32 = 12;

const RMDIR: u32 = 13;

const RENAME: u32 = 14;

const LINK: u32 = 15;

const READDIR: u32 = 16;

const READDIRPLUS: u32 = 17;

const FSSTAT: u32 = 18;

const FSINFO: u32 = 19;

const PATHCONF: u32 = 20;

const COMMIT: u32 = 21;

#[cfg_attr(feature = "arbitrary", derive(Clone, Debug))]
pub struct RpcRequest {
    // calculated
    pub size: u32,
    pub xid: u32,
    // always
    pub request: u32,
    // always 2
    pub rpc_version: u32,
    pub prog: u32,
    // both mount and nfs - 3
    pub version: u32,
    pub proc: u32,
    // for now only None (0)
    pub auth: u32,
    // for now only None (0)
    pub auth_verf: u32,
    pub args: Arguments,
}

#[cfg(feature = "arbitrary")]
impl<'a> Arbitrary<'a> for RpcRequest {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let prog = u.int_in_range(NFS_PROGRAM..=MOUNT_PROGRAM)?;
        let proc = u.int_in_range(0..=24)?;
        let args = match (prog, proc) {
            (NFS_PROGRAM, NULL) => Arguments::Null,
            (NFS_PROGRAM, GETATTR) => Arguments::GetAttr(u.arbitrary()?),
            (NFS_PROGRAM, SETATTR) => Arguments::SetAttr(u.arbitrary()?),
            (NFS_PROGRAM, LOOKUP) => Arguments::LookUp(u.arbitrary()?),
            (NFS_PROGRAM, ACCESS) => Arguments::Access(u.arbitrary()?),
            (NFS_PROGRAM, READLINK) => Arguments::ReadLink(u.arbitrary()?),
            (NFS_PROGRAM, READ) => Arguments::Read(u.arbitrary()?),
            (NFS_PROGRAM, WRITE) => Arguments::Write(u.arbitrary()?),
            (NFS_PROGRAM, CREATE) => Arguments::Create(u.arbitrary()?),
            (NFS_PROGRAM, MKDIR) => Arguments::MkDir(u.arbitrary()?),
            (NFS_PROGRAM, SYMLINK) => Arguments::SymLink(u.arbitrary()?),
            (NFS_PROGRAM, MKNOD) => Arguments::MkNod(u.arbitrary()?),
            (NFS_PROGRAM, REMOVE) => Arguments::Remove(u.arbitrary()?),
            (NFS_PROGRAM, RMDIR) => Arguments::RmDir(u.arbitrary()?),
            (NFS_PROGRAM, RENAME) => Arguments::Rename(u.arbitrary()?),
            (NFS_PROGRAM, LINK) => Arguments::Link(u.arbitrary()?),
            (NFS_PROGRAM, READDIR) => Arguments::ReadDir(u.arbitrary()?),
            (NFS_PROGRAM, READDIRPLUS) => Arguments::ReadDirPlus(u.arbitrary()?),
            (NFS_PROGRAM, FSSTAT) => Arguments::FsStat(u.arbitrary()?),
            (NFS_PROGRAM, FSINFO) => Arguments::FsInfo(u.arbitrary()?),
            (NFS_PROGRAM, PATHCONF) => Arguments::PathConf(u.arbitrary()?),
            (NFS_PROGRAM, COMMIT) => Arguments::Commit(u.arbitrary()?),
            (MOUNT_PROGRAM, 0) => Arguments::Null,
            (MOUNT_PROGRAM, 1) => Arguments::Mount(u.arbitrary()?),
            (MOUNT_PROGRAM, 2) => Arguments::Dump,
            (MOUNT_PROGRAM, 3) => Arguments::Unmount(u.arbitrary()?),
            (MOUNT_PROGRAM, 4) => Arguments::UnmountAll,
            (MOUNT_PROGRAM, 5) => Arguments::Export,
            _ => u.arbitrary::<Arguments>()?,
        };
        Ok(Self {
            size: 0,
            xid: 0,
            request: 0,
            //so there would be RpcVersionMismatch
            rpc_version: u.int_in_range(1..=2)?,
            //so there would be ProgramMismatch
            prog,
            //so there would be ProgramVersionMismatch
            version: u.int_in_range(2..=3)?,
            //so there would be ProcedureMismatch (nfsv3 has 21 proc)
            proc,
            auth: 0,
            auth_verf: 0,
            args,
        })
    }
}
