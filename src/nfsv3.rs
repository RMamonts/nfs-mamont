#![allow(non_camel_case_types, clippy::upper_case_acronyms)]

use num_derive::FromPrimitive;

#[allow(dead_code)]
pub(crate) const NFS_PROGRAM: u32 = 100003;
#[allow(dead_code)]
pub(crate) const NFS_VERSION: u32 = 3;
#[allow(dead_code)]
const NFS3_FHSIZE: u32 = 8;
#[allow(dead_code)]
const NFS3_COOKIEVERFSIZE: u32 = 8;
#[allow(dead_code)]
pub const NFS3_CREATEVERFSIZE: u32 = 8;
#[allow(dead_code)]
const NFS3_WRITEVERFSIZE: u32 = 8;
#[allow(dead_code)]
const MAX_FILENAME_SIZE: u32 = 255;
#[allow(dead_code)]
const MAX_PATH_SIZE: u32 = 255;

#[allow(dead_code)]
#[repr(u32)]
enum NFS_V3 {
    NFSPROC3_NULL = 0,
    NFSPROC3_GETATTR(GETATTR3args) = 1,
    NFSPROC3_SETATTR(SETATTR3args) = 2,
    NFSPROC3_LOOKUP(LOOKUP3args) = 3,
    NFSPROC3_ACCESS(ACCESS3args) = 4,
    NFSPROC3_READLINK(READLINK3args) = 5,
    NFSPROC3_READ(READ3args) = 6,
    NFSPROC3_WRITE(WRITE3args) = 7,
    NFSPROC3_CREATE(CREATE3args) = 8,
    NFSPROC3_MKDIR(MKDIR3args) = 9,
    NFSPROC3_SYMLINK(SYMLINK3args) = 10,
    NFSPROC3_MKNOD(MKNOD3args) = 11,
    NFSPROC3_REMOVE(REMOVE3args) = 12,
    NFSPROC3_RMDIR(RMDIR3args) = 13,
    NFSPROC3_RENAME(RENAME3args) = 14,
    NFSPROC3_LINK(LINK3args) = 15,
    NFSPROC3_READDIR(READDIR3args) = 16,
    NFSPROC3_READDIRPLUS(READDIRPLUS3args) = 17,
    NFSPROC3_FSSTAT(FSSTAT3args) = 18,
    NFSPROC3_FSINFO(FSINFOargs) = 19,
    NFSPROC3_PATHCONF(PATHCONF3args) = 20,
    NFSPROC3_COMMIT(COMMIT3args) = 21,
}

#[allow(dead_code)]
type filename3 = String;
#[allow(dead_code)]
type nfspath3 = String;
type fileid3 = u64;
type cookie3 = u64;
type cookieverf3 = [u8; NFS3_COOKIEVERFSIZE as usize];
type createverf3 = [u8; NFS3_CREATEVERFSIZE as usize];
type writeverf3 = [u8; NFS3_WRITEVERFSIZE as usize];
type uid3 = u32;
type gid3 = u32;
type size3 = u64;
type offset3 = u64;
type mode3 = u32;
type count3 = u32;

#[allow(dead_code)]
enum nfsstat3 {
    /// Indicates the call completed successfully.
    NFS3_OK = 0,
    /// Not owner. The operation was not allowed because the
    /// caller is either not a privileged user (root) or not the
    /// owner of the target of the operation.
    NFS3ERR_PERM = 1,
    /// No such file or directory. The file or directory name
    /// specified does not exist.
    NFS3ERR_NOENT = 2,
    /// I/O error. A hard error (for example, a disk error)
    /// occurred while processing the requested operation.
    NFS3ERR_IO = 5,
    /// I/O error. No such device or address.
    NFS3ERR_NXIO = 6,
    /// Permission denied. The caller does not have the correct
    /// permission to perform the requested operation. Contrast
    /// this with `NFS3ERR_PERM`, which restricts itself to owner
    /// or privileged user permission failures.
    NFS3ERR_ACCES = 13,
    /// File exists. The file specified already exists.
    NFS3ERR_EXIST = 17,
    /// Attempt to do a cross-device hard link.
    NFS3ERR_XDEV = 18,
    /// No such device.
    NFS3ERR_NODEV = 19,
    /// Not a directory. The caller specified a non-directory in
    /// a directory operation.
    NFS3ERR_NOTDIR = 20,
    /// Is a directory. The caller specified a directory in a
    /// non-directory operation.
    NFS3ERR_ISDIR = 21,
    /// Invalid argument or unsupported argument for an
    /// operation. Two examples are attempting a READLINK on an
    /// object other than a symbolic link or attempting to
    /// SETATTR a time field on a server that does not support
    /// this operation.
    NFS3ERR_INVAL = 22,
    /// File too large. The operation would have caused a file to
    /// grow beyond the server's limit.
    NFS3ERR_FBIG = 27,
    /// No space left on device. The operation would have caused
    /// the server's file system to exceed its limit.
    NFS3ERR_NOSPC = 28,
    /// Read-only file system. A modifying operation was
    /// attempted on a read-only file system.
    NFS3ERR_ROFS = 30,
    /// Too many hard links.
    NFS3ERR_MLINK = 31,
    /// The filename in an operation was too long.
    NFS3ERR_NAMETOOLONG = 63,
    /// An attempt was made to remove a directory that was not empty.
    NFS3ERR_NOTEMPTY = 66,
    /// Resource (quota) hard limit exceeded. The user's resource
    /// limit on the server has been exceeded.
    NFS3ERR_DQUOT = 69,
    /// Invalid file handle. The file handle given in the
    /// arguments was invalid. The file referred to by that file
    /// handle no longer exists or access to it has been
    /// revoked.
    NFS3ERR_STALE = 70,
    /// Too many levels of remote in path. The file handle given
    /// in the arguments referred to a file on a non-local file
    /// system on the server.
    NFS3ERR_REMOTE = 71,
    /// Illegal NFS file handle. The file handle failed internal
    /// consistency checks.
    NFS3ERR_BADHANDLE = 10001,
    /// Update synchronization mismatch was detected during a
    /// SETATTR operation.
    NFS3ERR_NOT_SYNC = 10002,
    /// READDIR or READDIRPLUS cookie is stale
    NFS3ERR_BAD_COOKIE = 10003,
    /// Operation is not supported.
    NFS3ERR_NOTSUPP = 10004,
    /// Buffer or request is too small.
    NFS3ERR_TOOSMALL = 10005,
    /// An error occurred on the server which does not map to any
    /// of the legal NFS version 3 protocol error values.  The
    /// client should translate this into an appropriate error.
    /// UNIX clients may choose to translate this to EIO.
    NFS3ERR_SERVERFAULT = 10006,
    /// An attempt was made to create an object of a type not
    /// supported by the server.
    NFS3ERR_BADTYPE = 10007,
    /// The server initiated the request, but was not able to
    /// complete it in a timely fashion. The client should wait
    /// and then try the request with a new RPC transaction ID.
    /// For example, this error should be returned from a server
    /// that supports hierarchical storage and receives a request
    /// to process a file that has been migrated. In this case,
    /// the server should start the immigration process and
    /// respond to client with this error.
    NFS3ERR_JUKEBOX = 10008,
}

#[allow(dead_code)]
#[derive(FromPrimitive)]
pub enum ftype3 {
    NF3REG = 1,
    NF3DIR = 2,
    NF3BLK = 3,
    NF3CHR = 4,
    NF3LNK = 5,
    NF3SOCK = 6,
    NF3FIFO = 7,
}

#[allow(dead_code)]
pub struct specdata3 {
    pub specdata1: u32,
    pub specdata2: u32,
}

#[allow(dead_code)]
pub struct nfs_fh3 {
    pub data: [u8; NFS3_FHSIZE as usize],
}

#[allow(dead_code)]
pub struct nfstime3 {
    pub seconds: u32,
    pub nseconds: u32,
}

#[allow(dead_code)]
struct fattr3 {
    ftype: ftype3,
    mode: mode3,
    nlink: u32,
    uid: uid3,
    gid: gid3,
    size: size3,
    used: size3,
    rdev: specdata3,
    fsid: u64,
    fileid: fileid3,
    atime: nfstime3,
    mtime: nfstime3,
    ctime: nfstime3,
}

type post_op_attr = Option<fattr3>;

#[allow(dead_code)]
struct wcc_attr {
    size: size3,
    mtime: nfstime3,
    ctime: nfstime3,
}

type pre_op_attr = Option<wcc_attr>;

#[allow(dead_code)]
struct wcc_data {
    before: pre_op_attr,
    after: post_op_attr,
}

type post_op_fh3 = Option<nfs_fh3>;
type set_mode3 = Option<mode3>;
type set_uid3 = Option<uid3>;
type set_gid3 = Option<gid3>;
type set_size3 = Option<size3>;

#[allow(dead_code)]
#[repr(u32)]
pub enum set_atime {
    DONT_CHANGE = 0,
    SET_TO_SERVER_TIME = 1,
    SET_TO_CLIENT_TIME(nfstime3) = 2,
}

#[allow(dead_code)]
#[repr(u32)]
pub enum set_mtime {
    DONT_CHANGE = 0,
    SET_TO_SERVER_TIME = 1,
    SET_TO_CLIENT_TIME(nfstime3) = 2,
}

#[allow(dead_code)]
pub struct sattr3 {
    pub mode: set_mode3,
    pub uid: set_uid3,
    pub gid: set_gid3,
    pub size: set_size3,
    pub atime: set_atime,
    pub mtime: set_mtime,
}

#[allow(dead_code)]
pub struct diropargs3 {
    pub dir: nfs_fh3,
    pub name: filename3,
}

#[allow(dead_code)]
struct GETATTR3args {
    object: nfs_fh3,
}

#[allow(dead_code)]
struct GETATTR3resok {
    obj_attributes: fattr3,
}

#[allow(dead_code)]
type sattrguard3 = Option<nfstime3>;

#[allow(dead_code)]
struct SETATTR3args {
    object: nfs_fh3,
    new_attribute: sattr3,
    guard: sattrguard3,
}

#[allow(dead_code)]
struct SETATTR3resok {
    obj_wcc: wcc_data,
}

#[allow(dead_code)]
struct SETATTR3resfail {
    obj_wcc: wcc_data,
}

#[allow(dead_code)]
struct LOOKUP3args {
    what: diropargs3,
}

#[allow(dead_code)]
struct LOOKUP3resok {
    object: nfs_fh3,
    obj_attributes: post_op_attr,
    dir_attributes: post_op_attr,
}

#[allow(dead_code)]
struct LOOKUP3resfail {
    dir_attributes: post_op_attr,
}

#[allow(dead_code)]
const ACCESS3_READ: u32 = 0x0001;
#[allow(dead_code)]
const ACCESS3_LOOKUP: u32 = 0x0002;
#[allow(dead_code)]
const ACCESS3_MODIFY: u32 = 0x0004;
#[allow(dead_code)]
const ACCESS3_EXTEND: u32 = 0x0008;
#[allow(dead_code)]
const ACCESS3_DELETE: u32 = 0x0010;
#[allow(dead_code)]
const ACCESS3_EXECUTE: u32 = 0x0020;

#[allow(dead_code)]
struct ACCESS3args {
    object: nfs_fh3,
    access: u32,
}

#[allow(dead_code)]
struct ACCESS3resok {
    obj_attributes: post_op_attr,
    access: u32,
}

#[allow(dead_code)]
struct ACCESS3resfail {
    obj_attributes: post_op_attr,
}

#[allow(dead_code)]
struct READLINK3args {
    symlink: nfs_fh3,
}

#[allow(dead_code)]
struct READLINK3resok {
    symlink_attributes: post_op_attr,
    data: nfspath3,
}

#[allow(dead_code)]
struct READLINK3resfail {
    symlink_attributes: post_op_attr,
}

#[allow(dead_code)]
struct READ3args {
    file: nfs_fh3,
    offset: offset3,
    count: count3,
}

#[allow(dead_code)]
struct READ3resok {
    file_attributes: post_op_attr,
    count: count3,
    eof: bool,
    data: Vec<u8>,
}

#[allow(dead_code)]
struct READ3resfail {
    file_attributes: post_op_attr,
}

#[allow(dead_code)]
#[derive(FromPrimitive)]
pub enum stable_how {
    UNSTABLE = 0,
    DATA_SYNC = 1,
    FILE_SYNC = 2,
}

#[allow(dead_code)]
struct WRITE3args {
    file: nfs_fh3,
    offset: offset3,
    count: count3,
    stable: stable_how,
    data: Vec<u8>,
}

#[allow(dead_code)]
struct WRITE3resok {
    file_wcc: wcc_data,
    count: count3,
    committed: stable_how,
    verf: writeverf3,
}

#[allow(dead_code)]
struct WRITE3resfail {
    file_wcc: wcc_data,
}

#[repr(u32)]
pub enum createhow3 {
    UNCHECKED(sattr3) = 0,
    GUARDED(sattr3) = 1,
    EXCLUSIVE(createverf3) = 2,
}

#[allow(dead_code)]
struct CREATE3args {
    dir_op: diropargs3,
    how: createhow3,
}

#[allow(dead_code)]
struct CREATE3resok {
    obj: post_op_fh3,
    obj_attributes: post_op_attr,
    dir_wcc: wcc_data,
}

#[allow(dead_code)]
struct CREATE3resfail {
    dir_wcc: wcc_data,
}

#[allow(dead_code)]
struct MKDIR3args {
    dir_op: diropargs3,
    attributes: sattr3,
}

#[allow(dead_code)]
struct MKDIR3resok {
    obj: post_op_fh3,
    obj_attributes: post_op_attr,
    dir_wcc: wcc_data,
}

#[allow(dead_code)]
struct MKDIR3resfail {
    dir_wcc: wcc_data,
}

#[allow(dead_code)]
pub struct symlinkdata3 {
    pub symlink_attributes: sattr3,
    pub symlink_data: nfspath3,
}

#[allow(dead_code)]
struct SYMLINK3args {
    dir_op: diropargs3,
    symlink: symlinkdata3,
}

#[allow(dead_code)]
struct SYMLINK3resok {
    obj: post_op_fh3,
    obj_attributes: post_op_attr,
    dir_wcc: wcc_data,
}

#[allow(dead_code)]
struct SYMLINK3resfail {
    dir_wcc: wcc_data,
}

#[allow(dead_code)]
pub struct devicedata3 {
    pub dev_attributes: sattr3,
    pub spec: specdata3,
}

#[allow(dead_code)]
#[repr(u32)]
pub enum mknoddata3 {
    NF3REG = 1,
    NF3DIR = 2,
    NF3BLK(devicedata3) = 3,
    NF3CHR(devicedata3) = 4,
    NF3LNK = 5,
    NF3SOCK(sattr3) = 6,
    NF3FIFO(sattr3) = 7,
}

#[allow(dead_code)]
struct MKNOD3args {
    dir_op: diropargs3,
    what: mknoddata3,
}

#[allow(dead_code)]
struct MKNOD3resok {
    obj: post_op_fh3,
    obj_attributes: post_op_attr,
    dir_wcc: wcc_data,
}

#[allow(dead_code)]
struct MKNOD3resfail {
    dir_wcc: wcc_data,
}

#[allow(dead_code)]
struct REMOVE3args {
    object: diropargs3,
}

#[allow(dead_code)]
struct REMOVE3resok {
    dir_wcc: wcc_data,
}

#[allow(dead_code)]
struct REMOVE3resfail {
    dir_wcc: wcc_data,
}

#[allow(dead_code)]
struct RMDIR3args {
    object: diropargs3,
}

#[allow(dead_code)]
struct RMDIR3resok {
    dir_wcc: wcc_data,
}

#[allow(dead_code)]
struct RMDIR3resfail {
    dir_wcc: wcc_data,
}

#[allow(dead_code)]
struct RENAME3args {
    from: diropargs3,
    to: diropargs3,
}

#[allow(dead_code)]
struct RENAME3resok {
    fromdir_wcc: wcc_data,
    todir_wcc: wcc_data,
}

#[allow(dead_code)]
struct RENAME3resfail {
    fromdir_wcc: wcc_data,
    todir_wcc: wcc_data,
}

#[allow(dead_code)]
struct LINK3args {
    file: nfs_fh3,
    link: diropargs3,
}

#[allow(dead_code)]
struct LINK3resok {
    file_attributes: post_op_attr,
    linkdir_wcc: wcc_data,
}

#[allow(dead_code)]
struct LINK3resfail {
    file_attributes: post_op_attr,
    linkdir_wcc: wcc_data,
}

#[allow(dead_code)]
struct READDIR3args {
    dir: nfs_fh3,
    cookie: cookie3,
    cookieverf: cookieverf3,
    count: count3,
}

#[allow(dead_code)]
struct entry3 {
    fileid: fileid3,
    name: filename3,
    cookie: cookie3,
    nextentry: Option<Box<entry3>>,
}

#[allow(dead_code)]
struct dirlist3 {
    entries: Option<Box<entry3>>,
    eof: bool,
}

#[allow(dead_code)]
struct READDIR3resok {
    dir_attributes: post_op_attr,
    cookieverf: cookieverf3,
    reply: dirlist3,
}

#[allow(dead_code)]
struct READDIR3resfail {
    dir_attributes: post_op_attr,
}

#[allow(dead_code)]
struct READDIRPLUS3args {
    dir: nfs_fh3,
    cookie: cookie3,
    cookieverf: cookieverf3,
    dircount: count3,
    maxcount: count3,
}

#[allow(dead_code)]
struct entryplus3 {
    fileid: fileid3,
    name: filename3,
    cookie: cookie3,
    name_attributes: post_op_attr,
    name_handle: post_op_fh3,
    nextentry: Option<Box<entryplus3>>,
}

#[allow(dead_code)]
struct dirlistplus3 {
    entries: Option<Box<entryplus3>>,
    eof: bool,
}

#[allow(dead_code)]
struct READDIRPLUS3resok {
    dir_attributes: post_op_attr,
    cookieverf: cookieverf3,
    reply: dirlistplus3,
}

#[allow(dead_code)]
struct READDIRPLUS3resfail {
    dir_attributes: post_op_attr,
}

#[allow(dead_code)]
struct FSSTAT3args {
    fsroot: nfs_fh3,
}

#[allow(dead_code)]
struct FSSTAT3resok {
    obj_attributes: post_op_attr,
    tbytes: size3,
    fbytes: size3,
    abytes: size3,
    tfiles: size3,
    ffiles: size3,
    afiles: size3,
    invarsec: u32,
}

#[allow(dead_code)]
struct FSSTAT3resfail {
    obj_attributes: post_op_attr,
}

#[allow(dead_code)]
const FSF3_LINK: u32 = 0x0001;
#[allow(dead_code)]
const FSF3_SYMLINK: u32 = 0x0002;
#[allow(dead_code)]
const FSF3_HOMOGENEOUS: u32 = 0x0008;
#[allow(dead_code)]
const FSF3_CANSETTIME: u32 = 0x0010;

#[allow(dead_code)]
struct FSINFOargs {
    fsroot: nfs_fh3,
}

#[allow(dead_code)]
struct FSINFO3resok {
    obj_attributes: post_op_attr,
    rtmax: u32,
    rtpref: u32,
    rtmult: u32,
    wtmax: u32,
    wtpref: u32,
    wtmult: u32,
    dtpref: u32,
    maxfilesize: size3,
    time_delta: nfstime3,
    properties: u32,
}

#[allow(dead_code)]
struct FSINFO3resfail {
    obj_attributes: post_op_attr,
}

#[allow(dead_code)]
struct PATHCONF3args {
    object: nfs_fh3,
}

#[allow(dead_code)]
struct PATHCONF3resok {
    obj_attributes: post_op_attr,
    linkmax: u32,
    name_max: u32,
    no_trunc: bool,
    chown_restricted: bool,
    case_insensitive: bool,
    case_preserving: bool,
}

#[allow(dead_code)]
struct PATHCONF3resfail {
    obj_attributes: post_op_attr,
}

#[allow(dead_code)]
struct COMMIT3args {
    file: nfs_fh3,
    offset: offset3,
    count: count3,
}

#[allow(dead_code)]
struct COMMIT3resok {
    file_wcc: wcc_data,
    verf: writeverf3,
}

#[allow(dead_code)]
struct COMMIT3resfail {
    file_wcc: wcc_data,
}
