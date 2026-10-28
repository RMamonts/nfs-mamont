use num_derive::{FromPrimitive, ToPrimitive};

pub const NFS_PROGRAM: u32 = 100003;
pub const NFS_VERSION: u32 = 3;

/// NFSv3 procedures (RFC 1813 §3.3) as a type-safe enum.
#[derive(Debug, Copy, Clone, PartialEq, Eq, ToPrimitive, FromPrimitive)]
pub enum NfsProc {
    Null = 0,
    GetAttr = 1,
    SetAttr = 2,
    LookUp = 3,
    Access = 4,
    ReadLink = 5,
    Read = 6,
    Write = 7,
    Create = 8,
    MkDir = 9,
    SymLink = 10,
    MkNod = 11,
    Remove = 12,
    RmDir = 13,
    Rename = 14,
    Link = 15,
    ReadDir = 16,
    ReadDirPlus = 17,
    FsStat = 18,
    FsInfo = 19,
    PathConf = 20,
    Commit = 21,
}

/// Size of a file handle in bytes.
///
/// The first byte carries the [`crate::BackendId`] of the backend owning the object,
/// the rest identifies the object inside that backend. The value is not a multiple of
/// the XDR alignment; padding is added by the serializer and skipped by the parser.
pub const NFS3_FHSIZE: usize = 9;

pub const NFS3_COOKIEVERFSIZE: usize = 8;

pub const NFS3_CREATEVERFSIZE: usize = 8;

pub const NFS3_WRITEVERFSIZE: usize = 8;
