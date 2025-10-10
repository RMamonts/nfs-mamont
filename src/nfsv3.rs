#![allow(dead_code)]
#![allow(non_camel_case_types, clippy::upper_case_acronyms)]

const NFS_PROGRAM: u32 = 100003;
const NFS_VERSION: u32 = 3;
const NFS3_FHSIZE: u32 = 64;
const NFS3_COOKIEVERFSIZE: u32 = 8;
const NFS3_CREATEVERFSIZE: u32 = 8;
const NFS3_WRITEVERFSIZE: u32 = 8;

enum NFSProgram {
    NFSPROC3_NULL = 0,
    NFSPROC3_GETATTR = 1,
    NFSPROC3_SETATTR = 2,
    NFSPROC3_LOOKUP = 3,
    NFSPROC3_ACCESS = 4,
    NFSPROC3_READLINK = 5,
    NFSPROC3_READ = 6,
    NFSPROC3_WRITE = 7,
    NFSPROC3_CREATE = 8,
    NFSPROC3_MKDIR = 9,
    NFSPROC3_SYMLINK = 10,
    NFSPROC3_MKNOD = 11,
    NFSPROC3_REMOVE = 12,
    NFSPROC3_RMDIR = 13,
    NFSPROC3_RENAME = 14,
    NFSPROC3_LINK = 15,
    NFSPROC3_READDIR = 16,
    NFSPROC3_READDIRPLUS = 17,
    NFSPROC3_FSSTAT = 18,
    NFSPROC3_FSINFO = 19,
    NFSPROC3_PATHCONF = 20,
    NFSPROC3_COMMIT = 21,
}

type filename3 = Vec<u8>;
type nfspath3 = Vec<u8>;
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

#[derive(Debug)]
#[repr(u32)]
enum ftype3 {
    NF3REG = 1,
    NF3DIR = 2,
    NF3BLK = 3,
    NF3CHR = 4,
    NF3LNK = 5,
    NF3SOCK = 6,
    NF3FIFO = 7,
}

#[derive(Debug)]
struct specdata3 {
    specdata1: u32,
    specdata2: u32,
}

#[derive(Debug)]
struct nfs_fh3 {
    data: Vec<u8>,
}

#[derive(Debug)]
struct nfstime3 {
    seconds: u32,
    nseconds: u32,
}

#[derive(Debug)]
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

#[derive(Debug)]
struct wcc_attr {
    size: size3,
    mtime: nfstime3,
    ctime: nfstime3,
}

type pre_op_attr = Option<wcc_attr>;

#[derive(Debug)]
struct wcc_data {
    before: pre_op_attr,
    after: post_op_attr,
}

type post_op_fh3 = Option<nfs_fh3>;
type set_mode3 = Option<mode3>;
type set_uid3 = Option<uid3>;
type set_gid3 = Option<gid3>;
type set_size3 = Option<size3>;

#[derive(Debug)]
#[repr(u32)]
enum set_atime {
    DONT_CHANGE = 0,
    SET_TO_SERVER_TIME = 1,
    SET_TO_CLIENT_TIME(nfstime3) = 2,
}

#[derive(Debug)]
#[repr(u32)]
enum set_mtime {
    DONT_CHANGE = 0,
    SET_TO_SERVER_TIME = 1,
    SET_TO_CLIENT_TIME(nfstime3) = 2,
}

#[derive(Debug)]
struct sattr3 {
    mode: set_mode3,
    uid: set_uid3,
    gid: set_gid3,
    size: set_size3,
    atime: set_atime,
    mtime: set_mtime,
}

#[derive(Debug)]
struct diropargs3 {
    dir: nfs_fh3,
    name: filename3,
}

#[derive(Debug)]
struct GETATTR3args {
    object: nfs_fh3,
}

struct GETATTR3resok {
    obj_attributes: fattr3,
}

type sattrguard3 = Option<nfstime3>;

#[derive(Debug)]
struct SETATTR3args {
    object: nfs_fh3,
    new_attribute: sattr3,
    guard: Option<nfstime3>,
}

#[derive(Debug)]
struct SETATTR3resok {
    obj_wcc: wcc_data,
}

#[derive(Debug)]
struct SETATTR3resfail {
    obj_wcc: wcc_data,
}

#[derive(Debug)]
struct LOOKUP3args {
    what: diropargs3,
}

#[derive(Debug)]
struct LOOKUP3resok {
    object: nfs_fh3,
    obj_attributes: post_op_attr,
    dir_attributes: post_op_attr,
}

#[derive(Debug)]
struct LOOKUP3resfail {
    dir_attributes: post_op_attr,
}

const ACCESS3_READ: u32 = 0x0001;
const ACCESS3_LOOKUP: u32 = 0x0002;
const ACCESS3_MODIFY: u32 = 0x0004;
const ACCESS3_EXTEND: u32 = 0x0008;
const ACCESS3_DELETE: u32 = 0x0010;
const ACCESS3_EXECUTE: u32 = 0x0020;

#[derive(Debug)]
struct ACCESS3args {
    object: nfs_fh3,
    access: u32,
}

#[derive(Debug)]
struct ACCESS3resok {
    obj_attributes: post_op_attr,
    access: u32,
}

#[derive(Debug)]
struct ACCESS3resfail {
    obj_attributes: post_op_attr,
}

#[derive(Debug)]
struct READLINK3args {
    symlink: nfs_fh3,
}

#[derive(Debug)]
struct READLINK3resok {
    symlink_attributes: post_op_attr,
    data: nfspath3,
}

#[derive(Debug)]
struct READLINK3resfail {
    symlink_attributes: post_op_attr,
}

#[derive(Debug)]
struct READ3args {
    file: nfs_fh3,
    offset: offset3,
    count: count3,
}

#[derive(Debug)]
struct READ3resok {
    file_attributes: post_op_attr,
    count: count3,
    eof: bool,
    data: Vec<u8>,
}

#[derive(Debug)]
struct READ3resfail {
    file_attributes: post_op_attr,
}

#[derive(Debug)]
enum stable_how {
    UNSTABLE = 0,
    DATA_SYNC = 1,
    FILE_SYNC = 2,
}

#[derive(Debug)]
struct WRITE3args {
    file: nfs_fh3,
    offset: offset3,
    count: count3,
    stable: stable_how,
    data: Vec<u8>,
}

#[derive(Debug)]
struct WRITE3resok {
    file_wcc: wcc_data,
    count: count3,
    committed: stable_how,
    verf: writeverf3,
}

#[derive(Debug)]
struct WRITE3resfail {
    file_wcc: wcc_data,
}

#[derive(Debug)]
#[repr(u32)]
enum createhow3 {
    UNCHECKED(sattr3) = 0,
    GUARDED(sattr3) = 1,
    EXCLUSIVE(createverf3) = 2,
}

#[derive(Debug)]
struct CREATE3args {
    dir_op: diropargs3,
    how: createhow3,
}

#[derive(Debug)]
struct CREATE3resok {
    obj: post_op_fh3,
    obj_attributes: post_op_attr,
    dir_wcc: wcc_data,
}

#[derive(Debug)]
struct CREATE3resfail {
    dir_wcc: wcc_data,
}

#[derive(Debug)]
struct MKDIR3args {
    dir_op: diropargs3,
    attributes: sattr3,
}

#[derive(Debug)]
struct MKDIR3resok {
    obj: post_op_fh3,
    obj_attributes: post_op_attr,
    dir_wcc: wcc_data,
}

#[derive(Debug)]
struct MKDIR3resfail {
    dir_wcc: wcc_data,
}

#[derive(Debug)]
struct symlinkdata3 {
    symlink_attributes: sattr3,
    symlink_data: nfspath3,
}

#[derive(Debug)]
struct SYMLINK3args {
    dir_op: diropargs3,
    symlink: symlinkdata3,
}

#[derive(Debug)]
struct SYMLINK3resok {
    obj: post_op_fh3,
    obj_attributes: post_op_attr,
    dir_wcc: wcc_data,
}

#[derive(Debug)]
struct SYMLINK3resfail {
    dir_wcc: wcc_data,
}

#[derive(Debug)]
struct devicedata3 {
    dev_attributes: sattr3,
    spec: specdata3,
}

#[derive(Debug)]
#[repr(u32)]
enum mknoddata3 {
    NF3REG = 1,
    NF3DIR = 2,
    NF3BLK(devicedata3) = 3,
    NF3CHR(devicedata3) = 4,
    NF3LNK = 5,
    NF3SOCK(sattr3) = 6,
    NF3FIFO(sattr3) = 7,
}

#[derive(Debug)]
struct MKNOD3args {
    dir_op: diropargs3,
    what: mknoddata3,
}

#[derive(Debug)]
struct MKNOD3resok {
    obj: post_op_fh3,
    obj_attributes: post_op_attr,
    dir_wcc: wcc_data,
}

#[derive(Debug)]
struct MKNOD3resfail {
    dir_wcc: wcc_data,
}

#[derive(Debug)]
struct REMOVE3args {
    object: diropargs3,
}

#[derive(Debug)]
struct REMOVE3resok {
    dir_wcc: wcc_data,
}

#[derive(Debug)]
struct REMOVE3resfail {
    dir_wcc: wcc_data,
}

#[derive(Debug)]
struct RMDIR3args {
    object: diropargs3,
}

#[derive(Debug)]
struct RMDIR3resok {
    dir_wcc: wcc_data,
}

#[derive(Debug)]
struct RMDIR3resfail {
    dir_wcc: wcc_data,
}

#[derive(Debug)]
struct RENAME3args {
    from: diropargs3,
    to: diropargs3,
}

#[derive(Debug)]
struct RENAME3resok {
    fromdir_wcc: wcc_data,
    todir_wcc: wcc_data,
}

#[derive(Debug)]
struct RENAME3resfail {
    fromdir_wcc: wcc_data,
    todir_wcc: wcc_data,
}

#[derive(Debug)]
struct LINK3args {
    file: nfs_fh3,
    link: diropargs3,
}

#[derive(Debug)]
struct LINK3resok {
    file_attributes: post_op_attr,
    linkdir_wcc: wcc_data,
}

#[derive(Debug)]
struct LINK3resfail {
    file_attributes: post_op_attr,
    linkdir_wcc: wcc_data,
}

#[derive(Debug)]
struct READDIR3args {
    dir: nfs_fh3,
    cookie: cookie3,
    cookieverf: cookieverf3,
    count: count3,
}

#[derive(Debug)]
struct entry3 {
    fileid: fileid3,
    name: filename3,
    cookie: cookie3,
    nextentry: Option<Box<entry3>>,
}

#[derive(Debug)]
struct dirlist3 {
    entries: Option<Box<entry3>>,
    eof: bool,
}

#[derive(Debug)]
struct READDIR3resok {
    dir_attributes: post_op_attr,
    cookieverf: cookieverf3,
    reply: dirlist3,
}

#[derive(Debug)]
struct READDIR3resfail {
    dir_attributes: post_op_attr,
}

#[derive(Debug)]
struct READDIRPLUS3args {
    dir: nfs_fh3,
    cookie: cookie3,
    cookieverf: cookieverf3,
    dircount: count3,
    maxcount: count3,
}

#[derive(Debug)]
struct entryplus3 {
    fileid: fileid3,
    name: filename3,
    cookie: cookie3,
    name_attributes: post_op_attr,
    name_handle: post_op_fh3,
    nextentry: Option<Box<entryplus3>>,
}

#[derive(Debug)]
struct dirlistplus3 {
    entries: Option<Box<entryplus3>>,
    eof: bool,
}

#[derive(Debug)]
struct READDIRPLUS3resok {
    dir_attributes: post_op_attr,
    cookieverf: cookieverf3,
    reply: dirlistplus3,
}

#[derive(Debug)]
struct READDIRPLUS3resfail {
    dir_attributes: post_op_attr,
}

#[derive(Debug)]
struct FSSTAT3args {
    fsroot: nfs_fh3,
}

#[derive(Debug)]
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

#[derive(Debug)]
struct FSSTAT3resfail {
    obj_attributes: post_op_attr,
}

const FSF3_LINK: u32 = 0x0001;
const FSF3_SYMLINK: u32 = 0x0002;
const FSF3_HOMOGENEOUS: u32 = 0x0008;
const FSF3_CANSETTIME: u32 = 0x0010;

#[derive(Debug)]
struct FSINFOargs {
    fsroot: nfs_fh3,
}

#[derive(Debug)]
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

#[derive(Debug)]
struct FSINFO3resfail {
    obj_attributes: post_op_attr,
}

#[derive(Debug)]
struct PATHCONF3args {
    object: nfs_fh3,
}

#[derive(Debug)]
struct PATHCONF3resok {
    obj_attributes: post_op_attr,
    linkmax: u32,
    name_max: u32,
    no_trunc: bool,
    chown_restricted: bool,
    case_insensitive: bool,
    case_preserving: bool,
}

#[derive(Debug)]
struct PATHCONF3resfail {
    obj_attributes: post_op_attr,
}

#[derive(Debug)]
struct COMMIT3args {
    file: nfs_fh3,
    offset: offset3,
    count: count3,
}

#[derive(Debug)]
struct COMMIT3resok {
    file_wcc: wcc_data,
    verf: writeverf3,
}

#[derive(Debug)]
struct COMMIT3resfail {
    file_wcc: wcc_data,
}
