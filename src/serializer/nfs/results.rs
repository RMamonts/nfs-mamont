//! XDR serialization for NFSv3 procedure results (RFC 1813).
//!
//! This module mirrors the parsing approach in `parser/nfsv3/procedures.rs`, but for
//! *responses*: it serializes NFSv3 result unions (e.g. `GETATTR3res`) and the
//! dependent structures needed by those results (`fattr3car`, `WccData`, directory
//! entry lists, etc.).
//!
//! Signatures generally take `crate::vfs::VfsResult<T>`-shaped values
//! (`Result<T, NfsError>`) and emit the on-the-wire union discriminant (`nfsstat3`)
//! followed by the `ok` or `fail` arm payload described by RFC 1813.
use std::io::{self, Write};

use crate::nfsv3::nfstime3;
use crate::serializer::nfs::{nfs_fh3, nfstime};
use crate::serializer::{array, bool, option, string_max_size, u32, u64};
use crate::vfs::{
    CookieVerifier, DirectoryEntry, DirectoryPlusEntry, FileAttr, FileHandle, FileName, FileTime,
    FileType, FsPath, NfsError, StableVerifier, WccData, WriteMode,
};

/// Maximum name length.
const MAX_NAME_LEN: usize = 255;

/// Maximum path length
const MAX_PATH_LEN: usize = 1024;

const WRITE_VERIFIER_SIZE: usize = 8;

const NFS3_COOKIEVERFSIZE: usize = 8;

// probably need come ADT to match all result:
// struct PRocRes {code: NfsError, res: Result<...Ok, ...fail>}

type Size = u32;

#[allow(dead_code)]
struct DirList {
    enrties: Vec<DirectoryEntry>,
    eof: bool,
}

#[allow(dead_code)]
struct DirListPlus {
    enrties: Vec<DirectoryPlusEntry>,
    eof: bool,
}

#[allow(dead_code)]
struct GetAttrResOk {
    pub object: FileAttr,
}

#[allow(dead_code)]
struct SetAttrResOk {
    obj_wcc: WccData,
}

#[allow(dead_code)]
struct SetAttrResFail {
    obj_wcc: WccData,
}

#[allow(dead_code)]
struct LookUpResOk {
    object: nfs_fh3,
    obj_attributes: Option<FileAttr>,
    dir_attributes: Option<FileAttr>,
}

#[allow(dead_code)]
struct LookUpResFail {
    dir_attributes: Option<FileAttr>,
}

#[allow(dead_code)]
struct AccessResOk {
    obj_attributes: Option<FileAttr>,
    access: u32,
}

#[allow(dead_code)]
struct AccessResFail {
    obj_attributes: Option<FileAttr>,
}

#[allow(dead_code)]
struct ReadLinkResOk {
    symlink_attributes: Option<FileAttr>,
    data: FsPath,
}

#[allow(dead_code)]
struct ReadLinkResFail {
    symlink_attributes: Option<FileAttr>,
}

#[allow(dead_code)]
struct ReadResOk {
    file_attributes: Option<FileAttr>,
    count: u32,
    eof: bool,
    //replace with Slice of allocator
    data: Vec<u8>,
}

#[allow(dead_code)]
struct ReadResFail {
    file_attributes: Option<FileAttr>,
}

#[allow(dead_code)]
struct WriteResOk {
    file_wcc: WccData,
    count: u32,
    committed: WriteMode,
    verf: [u8; WRITE_VERIFIER_SIZE],
}

#[allow(dead_code)]
struct WriteResFail {
    file_wcc: WccData,
}

#[allow(dead_code)]
struct CreateResOk {
    obj: Option<FileHandle>,
    obj_attributes: Option<FileAttr>,
    dir_wcc: WccData,
}

#[allow(dead_code)]
struct CreateResFail {
    dir_wcc: WccData,
}

#[allow(dead_code)]
struct MkdirResOk {
    obj: Option<FileHandle>,
    obj_attributes: Option<FileAttr>,
    dir_wcc: WccData,
}

#[allow(dead_code)]
struct MkdirResFail {
    dir_wcc: WccData,
}

#[allow(dead_code)]
struct SymlinkResOk {
    obj: Option<FileHandle>,
    obj_attributes: Option<FileAttr>,
    dir_wcc: WccData,
}

#[allow(dead_code)]
struct SymlinkResFail {
    dir_wcc: WccData,
}

#[allow(dead_code)]
struct MknodResOk {
    obj: Option<FileHandle>,
    obj_attributes: Option<FileAttr>,
    dir_wcc: WccData,
}

#[allow(dead_code)]
struct MknodResFail {
    dir_wcc: WccData,
}

#[allow(dead_code)]
struct RemoveResOk {
    dir_wcc: WccData,
}

#[allow(dead_code)]
struct RemoveResFail {
    dir_wcc: WccData,
}

#[allow(dead_code)]
struct RmdirResOk {
    dir_wcc: WccData,
}

#[allow(dead_code)]
struct RmdirResFail {
    dir_wcc: WccData,
}

#[allow(dead_code)]
struct RenameResOk {
    fromdir_wcc: WccData,
    todir_wcc: WccData,
}

#[allow(dead_code)]
struct RenameResFail {
    fromdir_wcc: WccData,
    todir_wcc: WccData,
}

#[allow(dead_code)]
struct LinkResOk {
    file_attributes: Option<FileAttr>,
    linkdir_wcc: WccData,
}

#[allow(dead_code)]
struct LinkResFail {
    file_attributes: Option<FileAttr>,
    linkdir_wcc: WccData,
}

#[allow(dead_code)]
struct ReadDirResOk {
    dir_attributes: Option<FileAttr>,
    cookieverf: [u8; NFS3_COOKIEVERFSIZE],
    reply: DirList,
}

#[allow(dead_code)]
struct ReadDirResFail {
    dir_attributes: Option<FileAttr>,
}

#[allow(dead_code)]
struct ReadDirPlusResOk {
    dir_attributes: Option<FileAttr>,
    cookieverf: [u8; NFS3_COOKIEVERFSIZE],
    reply: DirListPlus,
}

#[allow(dead_code)]
struct ReadDirPlusResFail {
    dir_attributes: Option<FileAttr>,
}

#[allow(dead_code)]
struct FsstatResOk {
    obj_attributes: Option<FileAttr>,
    tbytes: Size,
    fbytes: Size,
    abytes: Size,
    tfiles: Size,
    ffiles: Size,
    afiles: Size,
    invarsec: u32,
}

#[allow(dead_code)]
struct FsstatResFail {
    obj_attributes: Option<FileAttr>,
}

#[allow(dead_code)]
struct FsinfoResOk {
    obj_attributes: Option<FileAttr>,
    rtmax: u32,
    rtpref: u32,
    rtmult: u32,
    wtmax: u32,
    wtpref: u32,
    wtmult: u32,
    dtpref: u32,
    maxfilesize: Size,
    time_delta: nfstime3,
    properties: u32,
}

#[allow(dead_code)]
struct FsinfoResFail {
    obj_attributes: Option<FileAttr>,
}

#[allow(dead_code)]
struct PathConfResOk {
    obj_attributes: Option<FileAttr>,
    linkmax: u32,
    name_max: u32,
    no_trunc: bool,
    chown_restricted: bool,
    case_insensitive: bool,
    case_preserving: bool,
}

#[allow(dead_code)]
struct PathConfResFail {
    obj_attributes: Option<FileAttr>,
}

#[allow(dead_code)]
struct CommitResOk {
    file_wcc: WccData,
    verf: [u8; WRITE_VERIFIER_SIZE],
}

#[allow(dead_code)]
struct CommitResFail {
    file_wcc: WccData,
}

#[allow(dead_code)]
fn nfsstat3(dest: &mut dyn Write, err: NfsError) -> io::Result<()> {
    let code = match err {
        NfsError::Ok => 0,
        NfsError::Perm => 1,
        NfsError::NoEnt => 2,
        NfsError::Io => 5,
        NfsError::NxIo => 6,
        NfsError::Access => 13,
        NfsError::Exist => 17,
        NfsError::XDev => 18,
        NfsError::Nodev => 19,
        NfsError::NotDir => 20,
        NfsError::IsDir => 21,
        NfsError::Inval => 22,
        NfsError::FBig => 27,
        NfsError::NoSpc => 28,
        NfsError::RoFs => 30,
        NfsError::MLink => 31,
        NfsError::NameTooLong => 63,
        NfsError::NotEmpty => 66,
        NfsError::DQuot => 69,
        NfsError::Stale => 70,
        NfsError::Remote => 71,
        NfsError::BadCookie => 10003,
        NfsError::BadHandle => 10001,
        NfsError::NotSync => 10002,
        NfsError::NotSupp => 10004,
        NfsError::TooSmall => 10005,
        NfsError::ServerFault => 10006,
        NfsError::BadType => 10007,
        NfsError::Jukebox => 10008,
    };
    u32(dest, code)
}

fn ftype3(dest: &mut dyn Write, file_type: FileType) -> io::Result<()> {
    let type_code = match file_type {
        FileType::Regular => 1,
        FileType::Directory => 2,
        FileType::BlockDevice => 3,
        FileType::CharacterDevice => 4,
        FileType::Symlink => 5,
        FileType::Socket => 6,
        FileType::Fifo => 7,
    };
    u32(dest, type_code)
}

fn specdata3(dest: &mut dyn Write, major: u32, minor: u32) -> io::Result<()> {
    u32(dest, major).and_then(|_| u32(dest, minor))
}

fn nfstime3(dest: &mut dyn Write, file_time: FileTime) -> io::Result<()> {
    // Convert vfs::FileTime to nfsv3::nfstime3 and use the serializer from mod.rs
    let nfs_time = nfstime3 { seconds: file_time.seconds, nseconds: file_time.nanos };
    nfstime(dest, nfs_time)
}

fn fattr3(dest: &mut dyn Write, attr: FileAttr) -> io::Result<()> {
    let (major, minor) = attr.device.map(|d| (d.major, d.minor)).unwrap_or((0, 0));
    ftype3(dest, attr.file_type)?;
    u32(dest, attr.mode)?;
    u32(dest, attr.nlink)?;
    u32(dest, attr.uid)?;
    u32(dest, attr.gid)?;
    u64(dest, attr.size)?;
    u64(dest, attr.used)?;
    specdata3(dest, major, minor)?;
    u64(dest, attr.fsid)?;
    u64(dest, attr.fileid)?;
    nfstime3(dest, attr.atime)?;
    nfstime3(dest, attr.mtime)?;
    nfstime3(dest, attr.ctime)?;
    Ok(())
}

fn post_op_attr(dest: &mut dyn Write, attr: Option<FileAttr>) -> io::Result<()> {
    option(dest, attr, |attribute, dest| fattr3(dest, attribute))
}

fn wcc_attr(dest: &mut dyn Write, size: u64, mtime: FileTime, ctime: FileTime) -> io::Result<()> {
    u64(dest, size)?;
    nfstime3(dest, mtime)?;
    nfstime3(dest, ctime)?;
    Ok(())
}

fn pre_op_attr(dest: &mut dyn Write, before: Option<crate::vfs::AttrDigest>) -> io::Result<()> {
    option(dest, before, |digest, dest| wcc_attr(dest, digest.size, digest.mtime, digest.ctime))
}

#[allow(dead_code)]
fn wcc_data(dest: &mut dyn Write, wcc: WccData) -> io::Result<()> {
    pre_op_attr(dest, wcc.before)?;
    post_op_attr(dest, wcc.after)?;
    Ok(())
}

#[allow(dead_code)]
fn post_op_fh3(dest: &mut dyn Write, file_handle: Option<FileHandle>) -> io::Result<()> {
    option(dest, file_handle, |handle, dest| {
        // RFC 1813: nfs_fh3 is opaque<64>. Our VFS uses a fixed-size handle.
        nfs_fh3(dest, nfs_fh3 { data: handle.0 })
    })
}

#[allow(dead_code)]
fn stable_how(dest: &mut dyn Write, mode: WriteMode) -> io::Result<()> {
    let stability_code = match mode {
        WriteMode::Unstable => 0,
        WriteMode::DataSync => 1,
        WriteMode::FileSync => 2,
    };
    u32(dest, stability_code)
}

#[allow(dead_code)]
fn writeverf3(dest: &mut dyn Write, verifier: StableVerifier) -> io::Result<()> {
    array::<8>(dest, verifier.0)
}

#[allow(dead_code)]
fn cookieverf3(dest: &mut dyn Write, verifier: CookieVerifier) -> io::Result<()> {
    // RFC 1813 cookieverf3 is opaque[8]. We currently model it as u64.
    u64(dest, verifier.0)
}

fn filename3(dest: &mut dyn Write, name: FileName) -> io::Result<()> {
    string_max_size(dest, name.0, MAX_NAME_LEN)
}

#[allow(dead_code)]
fn nfspath3(dest: &mut dyn Write, path: String) -> io::Result<()> {
    string_max_size(dest, path, MAX_PATH_LEN)
}

#[allow(dead_code)]
// Linked-list encoding used by READDIR/READDIRPLUS:
// entry3list = *entry3
// entry3 = bool value_follows; if true => entry3 + next
fn entry3list(dest: &mut dyn Write, entries: Vec<DirectoryEntry>) -> io::Result<()> {
    for entry in entries {
        bool(dest, true)?;
        u64(dest, entry.fileid)?;
        filename3(dest, entry.name)?;
        u64(dest, entry.cookie.0)?;
    }
    bool(dest, false)
}

#[allow(dead_code)]
fn entryplus3list(dest: &mut dyn Write, entries: Vec<DirectoryPlusEntry>) -> io::Result<()> {
    for entry in entries {
        bool(dest, true)?;
        u64(dest, entry.fileid)?;
        filename3(dest, entry.name)?;
        u64(dest, entry.cookie.0)?;
        post_op_attr(dest, entry.attr)?;
        post_op_fh3(dest, entry.handle)?;
    }
    bool(dest, false)
}
