use std::io::Read;

use crate::nfsv3::{set_atime, set_mtime};
use crate::parser::nfsv3::{
    createhow3, diropargs3, mknoddata3, nfs_fh3, nfstime, sattr3, symlinkdata3, DirOpArg,
};
use crate::parser::primitive::{option, u32, u64};
use crate::parser::Result;
use crate::vfs::{
    AccessMask, CookieVerifier, CreateMode, DeviceId, DirectoryCookie, FileHandle, FileName,
    FileTime, FsPath, SetAttr, SetAttrGuard, SetTime, SpecialNode, WriteMode,
};

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct GetAttrArgs {
    pub object: FileHandle,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct SetAttrArgs {
    pub object: FileHandle,
    pub new_attribute: SetAttr,
    pub guard: SetAttrGuard,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct LookUpArgs {
    pub object: DirOpArg,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct AccessArgs {
    pub object: FileHandle,
    pub access: AccessMask,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct ReadLinkArgs {
    pub object: FileHandle,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct ReadArgs {
    pub object: FileHandle,
    pub offset: u64,
    pub count: u32,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct WriteArgs {
    pub object: FileHandle,
    pub offset: u64,
    pub count: u32,
    pub mode: WriteMode,
    pub data: Slice,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct CreateArgs {
    pub object: DirOpArg,
    pub mode: CreateMode,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct MkDirArgs {
    pub object: DirOpArg,
    pub attr: SetAttr,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct SymLinkArgs {
    pub object: DirOpArg,
    pub attr: SetAttr,
    pub path: FsPath,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct MkNodArgs {
    pub object: DirOpArg,
    pub mode: SpecialNode,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct RemoveArgs {
    pub object: DirOpArg,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct RmDirArgs {
    pub object: DirOpArg,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct RenameArgs {
    pub from: DirOpArg,
    pub to: DirOpArg,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct LinkArgs {
    pub object: FileHandle,
    pub link: DirOpArg,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct ReadDirArgs {
    pub object: FileHandle,
    pub cookie: DirectoryCookie,
    pub verf: CookieVerifier,
    pub count: u32,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct ReadDirPlusArgs {
    pub object: FileHandle,
    pub cookie: DirectoryCookie,
    pub verf: CookieVerifier,
    pub count: u32,
    pub max_count: u32,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct FsStatArgs {
    pub object: FileHandle,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct FsInfoArgs {
    pub object: FileHandle,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct PathConfArgs {
    pub object: FileHandle,
}

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct CommitArgs {
    pub object: FileHandle,
    pub offset: u64,
    pub count: u32,
}

pub fn access(src: &mut dyn Read) -> Result<AccessArgs> {
    Ok(AccessArgs { object: FileHandle(nfs_fh3(src)?.data), access: AccessMask(u32(src)?) })
}

pub fn get_attr(src: &mut dyn Read) -> Result<GetAttrArgs> {
    Ok(GetAttrArgs { object: FileHandle(nfs_fh3(src)?.data) })
}

pub fn set_attr(src: &mut dyn Read) -> Result<SetAttrArgs> {
    Ok(SetAttrArgs {
        object: FileHandle(nfs_fh3(src)?.data),
        new_attribute: convert_attr(sattr3(src)?),
        guard: match option(src, |s| nfstime(s))? {
            None => SetAttrGuard::None,
            Some(time) => SetAttrGuard::Check {
                ctime: FileTime { seconds: time.seconds, nanos: time.nseconds },
            },
        },
    })
}

pub fn lookup(src: &mut dyn Read) -> Result<LookUpArgs> {
    Ok(LookUpArgs { object: convert_diroparg(diropargs3(src)?) })
}

pub fn readlink(src: &mut dyn Read) -> Result<ReadLinkArgs> {
    Ok(ReadLinkArgs { object: FileHandle(nfs_fh3(src)?.data) })
}

pub fn read(src: &mut dyn Read) -> Result<ReadArgs> {
    Ok(ReadArgs {
        object: FileHandle(nfs_fh3(src)?.data),
        offset: u64(src)?,
        count: u32(src)?,
    })
}

pub fn write(_src: &mut dyn Read) -> Result<WriteArgs> {
    todo!()
}

pub fn create(src: &mut dyn Read) -> Result<CreateArgs> {
    Ok(CreateArgs {
        object: convert_diroparg(diropargs3(src)?),
        mode: match createhow3(src)? {
            createhow3::UNCHECKED(attr) => CreateMode::Unchecked { attr: convert_attr(attr) },
            createhow3::GUARDED(attr) => CreateMode::Guarded { attr: convert_attr(attr) },
            createhow3::EXCLUSIVE(verf) => CreateMode::Exclusive { verifier: verf },
        },
    })
}

pub fn mkdir(src: &mut dyn Read) -> Result<MkDirArgs> {
    Ok(MkDirArgs { object: convert_diroparg(diropargs3(src)?), attr: convert_attr(sattr3(src)?) })
}

pub fn symlink(src: &mut dyn Read) -> Result<SymLinkArgs> {
    let diropargs = diropargs3(src)?;
    let symlink = symlinkdata3(src)?;
    Ok(SymLinkArgs {
        object: convert_diroparg(diropargs),
        attr: convert_attr(symlink.symlink_attributes),
        path: FsPath(symlink.symlink_data),
    })
}

pub fn mknod(src: &mut dyn Read) -> Result<MkNodArgs> {
    Ok(MkNodArgs {
        object: convert_diroparg(diropargs3(src)?),
        mode: match mknoddata3(src)? {
            mknoddata3::NF3REG => SpecialNode::Regular,
            mknoddata3::NF3DIR => SpecialNode::Directory,
            mknoddata3::NF3BLK(data) => SpecialNode::Block {
                device: DeviceId { major: data.spec.specdata1, minor: data.spec.specdata2 },
                attr: convert_attr(data.dev_attributes),
            },
            mknoddata3::NF3CHR(data) => SpecialNode::Character {
                device: DeviceId { major: data.spec.specdata1, minor: data.spec.specdata2 },
                attr: convert_attr(data.dev_attributes),
            },
            mknoddata3::NF3LNK => SpecialNode::SymbolicLink,
            mknoddata3::NF3SOCK(attr) => SpecialNode::Socket { attr: convert_attr(attr) },
            mknoddata3::NF3FIFO(attr) => SpecialNode::Fifo { attr: convert_attr(attr) },
        },
    })
}

pub fn remove(src: &mut dyn Read) -> Result<RemoveArgs> {
    Ok(RemoveArgs { object: convert_diroparg(diropargs3(src)?) })
}

pub fn rmdir(src: &mut dyn Read) -> Result<RmDirArgs> {
    Ok(RmDirArgs { object: convert_diroparg(diropargs3(src)?) })
}

pub fn rename(src: &mut dyn Read) -> Result<RenameArgs> {
    Ok(RenameArgs {
        from: convert_diroparg(diropargs3(src)?),
        to: convert_diroparg(diropargs3(src)?),
    })
}

pub fn link(src: &mut dyn Read) -> Result<LinkArgs> {
    Ok(LinkArgs {
        object: FileHandle(nfs_fh3(src)?.data),
        link: convert_diroparg(diropargs3(src)?),
    })
}

pub fn readdir(src: &mut dyn Read) -> Result<ReadDirArgs> {
    Ok(ReadDirArgs {
        object: FileHandle(nfs_fh3(src)?.data),
        cookie: DirectoryCookie(u64(src)?),
        verf: CookieVerifier(u64(src)?),
        count: u32(src)?,
    })
}

pub fn readdir_plus(src: &mut dyn Read) -> Result<ReadDirPlusArgs> {
    Ok(ReadDirPlusArgs {
        object: FileHandle(nfs_fh3(src)?.data),
        cookie: DirectoryCookie(u64(src)?),
        verf: CookieVerifier(u64(src)?),
        count: u32(src)?,
        max_count: u32(src)?,
    })
}

pub fn fsstat(src: &mut dyn Read) -> Result<FsStatArgs> {
    Ok(FsStatArgs { object: FileHandle(nfs_fh3(src)?.data) })
}

pub fn fsinfo(src: &mut dyn Read) -> Result<FsInfoArgs> {
    Ok(FsInfoArgs { object: FileHandle(nfs_fh3(src)?.data) })
}

pub fn pathconf(src: &mut dyn Read) -> Result<PathConfArgs> {
    Ok(PathConfArgs { object: FileHandle(nfs_fh3(src)?.data) })
}

pub fn commit(src: &mut dyn Read) -> Result<CommitArgs> {
    Ok(CommitArgs {
        object: FileHandle(nfs_fh3(src)?.data),
        offset: u64(src)?,
        count: u32(src)?,
    })
}

fn convert_attr(attr: sattr3) -> SetAttr {
    let atime = match attr.atime {
        set_atime::DONT_CHANGE => SetTime::DontChange,
        set_atime::SET_TO_SERVER_TIME => SetTime::ServerCurrent,
        set_atime::SET_TO_CLIENT_TIME(time) => {
            SetTime::ClientProvided(FileTime { seconds: time.seconds, nanos: time.nseconds })
        }
    };
    let mtime = match attr.mtime {
        set_mtime::DONT_CHANGE => SetTime::DontChange,
        set_mtime::SET_TO_SERVER_TIME => SetTime::ServerCurrent,
        set_mtime::SET_TO_CLIENT_TIME(time) => {
            SetTime::ClientProvided(FileTime { seconds: time.seconds, nanos: time.nseconds })
        }
    };
    SetAttr { mode: attr.mode, uid: attr.uid, gid: attr.gid, size: attr.size, atime, mtime }
}

fn convert_diroparg(args: diropargs3) -> DirOpArg {
    DirOpArg { object: FileHandle(args.dir.data), name: FileName(args.name) }
}
