//! This module implements the `MOUNT` protocol (RFC 1813 Appendix I) data structures
//! for XDR serialization and deserialization.
//!
//! The `MOUNT` protocol is used to establish the initial connection between an NFS client
//! and server. It provides functions for mounting and unmounting file systems, and
//! obtaining the initial file handle that serves as the root of the mounted file system.

use std::io::{Read, Write};

use num_derive::{FromPrimitive, ToPrimitive};

use crate::xdr::{nfs3, Deserialize, DeserializeEnum, Serialize, SerializeEnum};
use crate::{DeserializeStruct, SerializeStruct};

/// MOUNT program number for RPC
pub const PROGRAM: u32 = 100005;
/// MOUNT protocol version 3
pub const VERSION: u32 = 3;

/// Maximum bytes in a path name
pub const MNTPATHLEN: u32 = 1024;
/// Maximum bytes in a name
pub const MNTNAMLEN: u32 = 255;
/// Maximum bytes in a V3 file handle
pub const FHSIZE3: u32 = 64;

/// File handle for NFS version 3
#[allow(non_camel_case_types)]
pub type fhandle3 = nfs3::nfs_fh3;
/// Directory path on the server
#[allow(non_camel_case_types)]
pub type dirpath = Vec<u8>;
/// Name in the directory
#[allow(non_camel_case_types)]
pub type name = Vec<u8>;

/// Status codes returned by `MOUNT` protocol operations
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum mountstat3 {
    MNT3_OK = 0,
    MNT3ERR_PERM = 1,
    MNT3ERR_NOENT = 2,
    MNT3ERR_IO = 5,
    MNT3ERR_ACCES = 13,
    MNT3ERR_NOTDIR = 20,
    MNT3ERR_INVAL = 22,
    MNT3ERR_NAMETOOLONG = 63,
    MNT3ERR_NOTSUPP = 10004,
    MNT3ERR_SERVERFAULT = 10006,
}
impl SerializeEnum for mountstat3 {}
impl DeserializeEnum for mountstat3 {}

/// Successful response to a mount request
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct mountres3_ok {
    pub fhandle: fhandle3,
    pub auth_flavors: Vec<u32>,
}
DeserializeStruct!(mountres3_ok, fhandle, auth_flavors);
SerializeStruct!(mountres3_ok, fhandle, auth_flavors);

/// Procedure numbers for the `MOUNT` version 3 protocol
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
pub enum MountProgram {
    MOUNTPROC3_NULL = 0,
    MOUNTPROC3_MNT = 1,
    MOUNTPROC3_DUMP = 2,
    MOUNTPROC3_UMNT = 3,
    MOUNTPROC3_UMNTALL = 4,
    MOUNTPROC3_EXPORT = 5,
    INVALID,
}
impl SerializeEnum for MountProgram {}
impl DeserializeEnum for MountProgram {}

pub enum Args {
    Null,
    Mnt(dirpath),
    Dump,
    Umnt(dirpath),
    Umntall,
    Export,
}
