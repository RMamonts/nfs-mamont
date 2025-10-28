use std::io::Read;
use crate::parser::nfsv3::DirOpArg;
use crate::parser::Result;
use crate::vfs::{AccessMask, CookieVerifier, CreateMode, DirectoryCookie, FileHandle, FsPath, SetAttr, SetAttrGuard, SpecialNode, WriteMode};

pub struct GetAttrArgs {
    object: FileHandle
}

pub struct SetAttrArgs {
    object: FileHandle,
    new_attribute: SetAttr,
    guard: SetAttrGuard
}

pub struct LookUpArgs {
    object: DirOpArg
}

pub struct AccessArgs {
    object: FileHandle,
    access: AccessMask,
}

pub struct ReadLinkArgs {
    object: FileHandle
}

pub struct ReadArgs {
    object: FileHandle,
    offset: u64,
    count: u32
}

pub struct WriteArgs {
    object: FileHandle,
    offset: u64,
    count: u32,
    mode: WriteMode,

    // change that to proper chain from allocator!!!
    data: Vec<u8>
}

pub struct CreateArgs {
    object: DirOpArg,
    mode: CreateMode,
}

pub struct MkDirArgs {
    object: DirOpArg,
    attr: SetAttr
}

pub struct SymLinkArgs {
    object: DirOpArg,
    attr: SetAttr,
    path: FsPath
}

pub struct MkNodArgs {
    object: DirOpArg,
    mode: SpecialNode,
}

pub struct RemoveArgs {
    object: DirOpArg
}

pub struct RmDirArgs {
    object: DirOpArg
}

pub struct RenameArgs {
    from: DirOpArg,
    to: DirOpArg
}

pub struct LinkArgs {
    object: FileHandle,
    link: DirOpArg,
}

pub struct ReadDirArgs {
    object: FileHandle,
    cookie: DirectoryCookie,
    verf: CookieVerifier,
    count: u32,
}

pub struct ReadDirPlusArgs {
    object: FileHandle,
    cookie: DirectoryCookie,
    verf: CookieVerifier,
    count: u32,
    max_count: u32,
}

pub struct FsStatArgs{
    object: FileHandle,
}

pub struct FsInfoArgs{
    object: FileHandle,
}

pub struct PathConfArgs{
    object: FileHandle,
}

pub struct CommitArgs {
    object: FileHandle,
    offset: u64,
    count: u32,
}

fn access(src: &mut impl Read) -> Result<AccessArgs> {
    todo!()
}

fn get_attr(src: &mut impl Read) -> Result<GetAttrArgs> {
    todo!()
}

fn set_attr(src: &mut impl Read) -> Result<SetAttrArgs> {
    todo!()
}

fn lookup(src: &mut impl Read) -> Result<LookUpArgs> {
    todo!()
}

fn readlink(src: &mut impl Read) -> Result<ReadLinkArgs> {
    todo!()
}

fn read(src: &mut impl Read) -> Result<ReadArgs> {
    todo!()
}

fn write(src: &mut impl Read) -> Result<WriteArgs> {
    todo!()
}

fn create(src: &mut impl Read) -> Result<CreateArgs> {
    todo!()
}

fn mkdir(src: &mut impl Read) -> Result<MkDirArgs> {
    todo!()
}

fn symlink(src: &mut impl Read) -> Result<SymLinkArgs> {
    todo!()
}

fn mknod(src: &mut impl Read) -> Result<MkNodArgs> {
    todo!()
}

fn remove(src: &mut impl Read) -> Result<RemoveArgs> {
    todo!()
}

fn rmdir(src: &mut impl Read) -> Result<RmDirArgs> {
    todo!()
}

fn rename(src: &mut impl Read) -> Result<RenameArgs> {
    todo!()
}

fn link(src: &mut impl Read) -> Result<LinkArgs> {
    todo!()
}

fn readdir(src: &mut impl Read) -> Result<ReadDirArgs> {
    todo!()
}

fn readdir_plus(src: &mut impl Read) -> Result<ReadDirPlusArgs> {
    todo!()
}

fn fsstat(src: &mut impl Read) -> Result<FsStatArgs> {
    todo!()
}

fn fsinfo(src: &mut impl Read) -> Result<FsInfoArgs> {
    todo!()
}

fn pathconf(src: &mut impl Read) -> Result<PathConfArgs> {
    todo!()
}

fn commit(src: &mut impl Read) -> Result<CommitArgs> {
    todo!()
}
