//! This module implements the `MOUNT` protocol (RFC 1813 Appendix I) data structures
//! for XDR serialization and deserialization.
//!
//! The `MOUNT` protocol is used to establish the initial connection between an NFS client
//! and server. It provides functions for mounting and unmounting file systems, and
//! obtaining the initial file handle that serves as the root of the mounted file system.

// Allow unused code since we implement the complete RFC specification
#![allow(dead_code)]
// Keep original RFC naming conventions for consistency with the specification

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
pub const FH_SIZE3: u32 = 64;

/// File handle for NFS version 3
pub type Fhandle3 = nfs3::NFSFh3;
/// Directory path on the server
pub type Dirpath = Vec<u8>;
/// Name in the directory
pub type Name = Vec<u8>;

/// Status codes returned by `MOUNT` protocol operations
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum MountStat3 {
    /// No error
    Mnt3Ok = 0, /* no error */
    /// Not owner
    Mnt3ErrPerm = 1, /* Not owner */
    /// No such file or directory
    Mnt3ErrNoEnt = 2, /* No such file or directory */
    /// I/O error
    Mnt3ErrIO = 5, /* I/O error */
    /// Permission denied
    Mnt3ErrAccess = 13, /* Permission denied */
    /// Not a directory
    Mnt3ErrNotDir = 20, /* Not a directory */
    /// Invalid argument
    Mnt3ErrInval = 22, /* Invalid argument */
    /// Filename too long
    Mnt3ErrNameTooLong = 63, /* Filename too long */
    /// Operation not supported
    Mnt3ErrNotSupp = 10004, /* Operation not supported */
    /// A failure on the server
    Mnt3ErrServerFault = 10006, /* A failure on the server */
}
impl SerializeEnum for MountStat3 {}
impl DeserializeEnum for MountStat3 {}

/// Successful response to a mount request
#[derive(Clone, Debug)]
pub struct MountRes3Ok {
    /// File handle for the mounted directory
    pub fhandle: Fhandle3, // really same thing as nfs::NFSFh3
    /// List of authentication flavors supported by the server
    pub auth_flavors: Vec<u32>,
}
DeserializeStruct!(MountRes3Ok, fhandle, auth_flavors);
SerializeStruct!(MountRes3Ok, fhandle, auth_flavors);

/// Procedure numbers for the `MOUNT` version 3 protocol
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
pub enum MountProgram {
    /// Null procedure for service availability testing
    MountProc3Null = 0,
    /// Mount a file system
    MountProc3Mnt = 1,
    /// Get list of mounted file systems
    MountProc3Dump = 2,
    /// Unmount a file system
    MountProc3Umnt = 3,
    /// Unmount all file systems
    MountProc3UmntAll = 4,
    /// Get list of exported file systems
    MountProc3Export = 5,
    /// Invalid procedure number
    Invalid,
}
impl SerializeEnum for MountProgram {}
impl DeserializeEnum for MountProgram {}
