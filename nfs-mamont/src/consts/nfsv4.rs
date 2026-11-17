use num_derive::{FromPrimitive, ToPrimitive};

/// NFSv3 procedures (RFC 7530 §15.2.2) as a type-safe enum.
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, ToPrimitive, FromPrimitive)]
pub enum Operation {
    Access = 3,
    Close = 4,
    Commit = 5,
    Create = 6,
    DelegPurge = 7,
    DelegReturn = 8,
    GetAttr = 9,
    GetFh = 10,
    Link = 11,
    Lock = 12,
    LockT = 13,
    LockU = 14,
    Lookup = 15,
    LookupP = 16,
    NVerify = 17,
    Open = 18,
    OpenAttr = 19,
    OpenConfirm = 20,
    OpenDowngrade = 21,
    PutFh = 22,
    PutPubFh = 23,
    PutRootFh = 24,
    Read = 25,
    ReadDir = 26,
    ReadLink = 27,
    Remove = 28,
    Rename = 29,
    Renew = 30,
    RestoreFh = 31,
    SaveFh = 32,
    Secinfo = 33,
    SetAttr = 34,
    SetClientId = 35,
    SetClientIdConfirm = 36,
    Verify = 37,
    Write = 38,
    ReleaseLockOwner = 39,
    Illegal = 10044,
}

/// Maximum size of a file handle (`nfs_fh4`), in bytes.
///
/// The NFSv4 specification (RFC 7530 §2.1) defines the `nfs_fh4` type as
/// `opaque nfs_fh4<NFS4_FHSIZE>` without fixing a numeric value in the text.
/// Implementations conventionally use 128 bytes; this crate follows suit.
pub const NFS4_FHSIZE: usize = 128;

/// Size of the `other` field of a [`crate::vfs::nfsv4::file::StateId`], in bytes.
pub const NFS4_OTHER_SIZE: usize = 12;

/// Size of a write verifier (`verifier4`), in bytes.
pub const NFS4_VERIFIER_SIZE: usize = 8;

/// Maximum length of filename.
pub const MAX_NAME_LEN: usize = 255;

/// Maximum length of opaque data, RFC 7531 §2
pub const NFS4_OPAQUE_LIMIT: usize = 1024;

/// Maximum length of ACE's owner
pub const NFS4_MAX_ACE_LEN: usize = 255;
