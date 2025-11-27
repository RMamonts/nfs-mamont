use std::io;
use std::io::{ErrorKind, Read};

use crate::allocator::Slice;
use crate::nfsv3::{set_atime, set_mtime, stable_how};
use crate::parser::nfsv3::{
    createhow3, diropargs3, mknoddata3, nfs_fh3, nfstime, sattr3, symlinkdata3, DirOpArg,
};
use crate::parser::primitive::{option, padding, u32, u32_as_usize, u64, variant};
use crate::parser::{Error, Result};
use crate::vfs::{
    AccessMask, CookieVerifier, CreateMode, DeviceId, DirectoryCookie, FileHandle, FileName,
    FileTime, FsPath, SetAttr, SetAttrGuard, SetTime, SpecialNode, WriteMode,
};
#[allow(dead_code)]
pub struct GetAttrArgs {
    object: FileHandle,
}

#[allow(dead_code)]
pub struct SetAttrArgs {
    object: FileHandle,
    new_attribute: SetAttr,
    guard: SetAttrGuard,
}

#[allow(dead_code)]
pub struct LookUpArgs {
    object: DirOpArg,
}

#[allow(dead_code)]
pub struct AccessArgs {
    object: FileHandle,
    access: AccessMask,
}

#[allow(dead_code)]
pub struct ReadLinkArgs {
    object: FileHandle,
}

#[allow(dead_code)]
pub struct ReadArgs {
    object: FileHandle,
    offset: u64,
    count: u32,
}

#[allow(dead_code)]
pub struct WriteArgs {
    object: FileHandle,
    offset: u64,
    count: u32,
    mode: WriteMode,
    data: Slice,
}

#[allow(dead_code)]
pub struct CreateArgs {
    object: DirOpArg,
    mode: CreateMode,
}

#[allow(dead_code)]
pub struct MkDirArgs {
    object: DirOpArg,
    attr: SetAttr,
}

#[allow(dead_code)]
pub struct SymLinkArgs {
    object: DirOpArg,
    attr: SetAttr,
    path: FsPath,
}

#[allow(dead_code)]
pub struct MkNodArgs {
    object: DirOpArg,
    mode: SpecialNode,
}

#[allow(dead_code)]
pub struct RemoveArgs {
    object: DirOpArg,
}

#[allow(dead_code)]
pub struct RmDirArgs {
    object: DirOpArg,
}

#[allow(dead_code)]
pub struct RenameArgs {
    from: DirOpArg,
    to: DirOpArg,
}

#[allow(dead_code)]
pub struct LinkArgs {
    object: FileHandle,
    link: DirOpArg,
}

#[allow(dead_code)]
pub struct ReadDirArgs {
    object: FileHandle,
    cookie: DirectoryCookie,
    verf: CookieVerifier,
    count: u32,
}

#[allow(dead_code)]
pub struct ReadDirPlusArgs {
    object: FileHandle,
    cookie: DirectoryCookie,
    verf: CookieVerifier,
    count: u32,
    max_count: u32,
}

#[allow(dead_code)]
pub struct FsStatArgs {
    object: FileHandle,
}

#[allow(dead_code)]
pub struct FsInfoArgs {
    object: FileHandle,
}

#[allow(dead_code)]
pub struct PathConfArgs {
    object: FileHandle,
}

#[allow(dead_code)]
pub struct CommitArgs {
    object: FileHandle,
    offset: u64,
    count: u32,
}

#[allow(dead_code)]
pub fn access(src: &mut dyn Read) -> Result<AccessArgs> {
    Ok(AccessArgs { object: FileHandle(nfs_fh3(src)?.data), access: AccessMask(u32(src)?) })
}

#[allow(dead_code)]
pub fn get_attr(src: &mut dyn Read) -> Result<GetAttrArgs> {
    Ok(GetAttrArgs { object: FileHandle(nfs_fh3(src)?.data) })
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn lookup(src: &mut dyn Read) -> Result<LookUpArgs> {
    Ok(LookUpArgs { object: convert_diroparg(diropargs3(src)?) })
}

#[allow(dead_code)]
pub fn readlink(src: &mut dyn Read) -> Result<ReadLinkArgs> {
    Ok(ReadLinkArgs { object: FileHandle(nfs_fh3(src)?.data) })
}

#[allow(dead_code)]
pub fn read(src: &mut dyn Read) -> Result<ReadArgs> {
    Ok(ReadArgs { object: FileHandle(nfs_fh3(src)?.data), offset: u64(src)?, count: u32(src)? })
}

#[allow(dead_code)]
pub fn write(src: &mut dyn Read, mut slice: Slice) -> Result<WriteArgs> {
    Ok(WriteArgs {
        object: FileHandle(nfs_fh3(src)?.data),
        offset: u64(src)?,
        count: u32(src)?,
        mode: write_mode(src)?,
        data: {
            read_in_slice(src, &mut slice)?;
            slice
        },
    })
}

// here comes slice of exact number of bytes we expect to write
// but current variant knows how to do it if we change fh to var size

pub fn read_in_slice(src: &mut dyn Read, slice: &mut Slice) -> Result<()> {
    let mut counter = 0;
    let size = u32_as_usize(src)?;
    for buf in slice.iter_mut() {
        src.read_exact(buf).map_err(Error::IO)?;
        counter += buf.len();
    }
    if counter != size {
        return Err(Error::IO(io::Error::new(
            ErrorKind::InvalidInput,
            "invalid amount of data read",
        )));
    }
    padding(src, counter)?;
    Ok(())
}

fn write_mode(src: &mut dyn Read) -> Result<WriteMode> {
    Ok(match variant::<stable_how>(src)? {
        stable_how::UNSTABLE => WriteMode::Unstable,
        stable_how::DATA_SYNC => WriteMode::DataSync,
        stable_how::FILE_SYNC => WriteMode::FileSync,
    })
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn mkdir(src: &mut dyn Read) -> Result<MkDirArgs> {
    Ok(MkDirArgs { object: convert_diroparg(diropargs3(src)?), attr: convert_attr(sattr3(src)?) })
}

#[allow(dead_code)]
pub fn symlink(src: &mut dyn Read) -> Result<SymLinkArgs> {
    let diropargs = diropargs3(src)?;
    let symlink = symlinkdata3(src)?;
    Ok(SymLinkArgs {
        object: convert_diroparg(diropargs),
        attr: convert_attr(symlink.symlink_attributes),
        path: FsPath(symlink.symlink_data),
    })
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn remove(src: &mut dyn Read) -> Result<RemoveArgs> {
    Ok(RemoveArgs { object: convert_diroparg(diropargs3(src)?) })
}

#[allow(dead_code)]
pub fn rmdir(src: &mut dyn Read) -> Result<RmDirArgs> {
    Ok(RmDirArgs { object: convert_diroparg(diropargs3(src)?) })
}

#[allow(dead_code)]
pub fn rename(src: &mut dyn Read) -> Result<RenameArgs> {
    Ok(RenameArgs {
        from: convert_diroparg(diropargs3(src)?),
        to: convert_diroparg(diropargs3(src)?),
    })
}

#[allow(dead_code)]
pub fn link(src: &mut dyn Read) -> Result<LinkArgs> {
    Ok(LinkArgs {
        object: FileHandle(nfs_fh3(src)?.data),
        link: convert_diroparg(diropargs3(src)?),
    })
}

#[allow(dead_code)]
pub fn readdir(src: &mut dyn Read) -> Result<ReadDirArgs> {
    Ok(ReadDirArgs {
        object: FileHandle(nfs_fh3(src)?.data),
        cookie: DirectoryCookie(u64(src)?),
        verf: CookieVerifier(u64(src)?),
        count: u32(src)?,
    })
}

#[allow(dead_code)]
pub fn readdir_plus(src: &mut dyn Read) -> Result<ReadDirPlusArgs> {
    Ok(ReadDirPlusArgs {
        object: FileHandle(nfs_fh3(src)?.data),
        cookie: DirectoryCookie(u64(src)?),
        verf: CookieVerifier(u64(src)?),
        count: u32(src)?,
        max_count: u32(src)?,
    })
}

#[allow(dead_code)]
pub fn fsstat(src: &mut dyn Read) -> Result<FsStatArgs> {
    Ok(FsStatArgs { object: FileHandle(nfs_fh3(src)?.data) })
}

#[allow(dead_code)]
pub fn fsinfo(src: &mut dyn Read) -> Result<FsInfoArgs> {
    Ok(FsInfoArgs { object: FileHandle(nfs_fh3(src)?.data) })
}

#[allow(dead_code)]
pub fn pathconf(src: &mut dyn Read) -> Result<PathConfArgs> {
    Ok(PathConfArgs { object: FileHandle(nfs_fh3(src)?.data) })
}

#[allow(dead_code)]
pub fn commit(src: &mut dyn Read) -> Result<CommitArgs> {
    Ok(CommitArgs { object: FileHandle(nfs_fh3(src)?.data), offset: u64(src)?, count: u32(src)? })
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
