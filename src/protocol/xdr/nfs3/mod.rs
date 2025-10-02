//! The module defines XDR data types and constants for Network File System (NFS)
//! version 3, as defined in RFC 1813.
//!
//! NFS version 3 is a stateless distributed file system protocol
//! that provides transparent remote access to shared file systems over a network.
//! This implementation provides the data structures needed for encoding and
//! decoding NFS version 3 protocol messages using XDR (External Data Representation).
//!
//! This module defines the constants, basic data types, and complex structures
//! that form the foundation of the `NFSv3` protocol as specified in RFC 1813.

// Allow unused code since we're implementing the full NFS3 protocol specification
#![allow(dead_code)]
// Preserve original RFC naming conventions for consistency with the specification

use std::fmt;
use std::io::{Read, Write};

use filetime;
use num_derive::{FromPrimitive, ToPrimitive};

use super::{deserialize, Deserialize, Serialize};
use crate::xdr::{DeserializeEnum, SerializeEnum, UsizeAsU32};
use crate::{xdr, DeserializeStruct, SerializeStruct};

// Modules for different operation types
pub mod dir;
pub mod file;
pub mod fs;

// Section 2.2 Constants
/// The RPC program number for NFS version 3 service.
pub const PROGRAM: u32 = 100_003;
/// The version number for NFS version 3 protocol.
pub const VERSION: u32 = 3;

// Section 2.4 Sizes
//
/// The maximum size in bytes of the opaque file handle.
pub const NFS3_FH_SIZE: u32 = 64;

/// The size in bytes of the opaque cookie verifier passed by
/// `READDIR` and `READDIRPLUS`.
pub const NFS3_COOKIE_VERF_SIZE: u32 = 8;

/// The size in bytes of the opaque verifier used for
/// exclusive `CREATE`.
pub const NFS3_CREATE_VERF_SIZE: u32 = 8;

/// The size in bytes of the opaque verifier used for
/// asynchronous `WRITE`.
pub const NFS3_WRITE_VERF_SIZE: u32 = 8;

// Section 2.5 Basic Data Types
/// A string type used in NFS for filenames and paths.
///
/// This is essentially a vector of bytes, but with specific
/// formatting for NFS protocol requirements.
#[derive(Default, Clone)]
pub struct NFSString(pub Vec<u8>);

impl NFSString {
    /// Returns the length of the string in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the string is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<u8>> for NFSString {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl From<&[u8]> for NFSString {
    fn from(value: &[u8]) -> Self {
        Self(value.into())
    }
}

impl AsRef<[u8]> for NFSString {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl std::ops::Deref for NFSString {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Debug for NFSString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(&self.0))
    }
}

impl fmt::Display for NFSString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(&self.0))
    }
}

impl Serialize for NFSString {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        self.0.serialize(dest)
    }
}

impl Deserialize for NFSString {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        Ok(NFSString(Deserialize::deserialize(src)?))
    }
}

/// Procedure numbers for NFS version 3 protocol.
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
pub enum NFSProgram {
    /// Do nothing - used primarily for performance measurement
    NFSProc3Null = 0,
    /// Get file attributes
    NFSProc3GetAttr = 1,
    /// Set file attributes
    NFSProc3SetAttr = 2,
    /// Look up filename
    NFSProc3Lookup = 3,
    /// Check file access permission
    NFSProc3Access = 4,
    /// Read from symbolic link
    NFSProc3ReadLink = 5,
    /// Read from file
    NFSProc3Read = 6,
    /// Write to file
    NFSProc3Write = 7,
    /// Create file
    NFSProc3Create = 8,
    /// Create directory
    NFSProc3MkDir = 9,
    /// Create symbolic link
    NFSProc3Symlink = 10,
    /// Create special device
    NFSProc3MkNod = 11,
    /// Remove file
    NFSProc3Remove = 12,
    /// Remove directory
    NFSProc3RmDir = 13,
    /// Rename file or directory
    NFSProc3Rename = 14,
    /// Create hard link
    NFSProc3Link = 15,
    /// Read directory
    NFSProc3ReadDir = 16,
    /// Extended read directory
    NFSProc3ReadDirPlus = 17,
    /// Get file system statistics
    NFSProc3FsStat = 18,
    /// Get file system information
    NFSProc3FsInfo = 19,
    /// Get path configuration
    NFSProc3PathConf = 20,
    /// Commit cached data
    NFSProc3Commit = 21,
    /// Invalid procedure
    Invalid = 22,
}

/// Opaque byte type as defined in RFC 1813 section 2.5
/// Used for binary data like file handles and verifiers
pub type Opaque = u8;
/// Filename type as defined in RFC 1813 section 2.5
/// String used for a component of a pathname
pub type Filename3 = NFSString;
/// Path type as defined in RFC 1813 section 2.5
/// String used for a pathname or a symbolic link contents
pub type NFSPath3 = NFSString;
/// File identifier as defined in RFC 1813 section 2.5
/// A unique number that identifies a file within a filesystem
pub type FileId3 = u64;
/// Directory entry position cookie as defined in RFC 1813 section 2.5
/// Used in `READDIR` and `READDIRPLUS` operations for iteration
pub type Cookie3 = u64;
/// Cookie verifier for directory operations as defined in RFC 1813 section 2.5
/// Used to detect when a directory being read has changed
pub type CookieVerf3 = [Opaque; NFS3_COOKIE_VERF_SIZE as usize];
/// Create verifier for exclusive file creation as defined in RFC 1813 section 2.5
/// Used in CREATE operations with `EXCLUSIVE` mode to ensure uniqueness
pub type CreateVerf3 = [Opaque; NFS3_CREATE_VERF_SIZE as usize];
/// Write verifier for asynchronous writes as defined in RFC 1813 section 2.5
/// Used to detect server reboots between asynchronous `WRITE` and `COMMIT` operations
pub type WriteVerf3 = [Opaque; NFS3_WRITE_VERF_SIZE as usize];
/// User ID as defined in RFC 1813 section 2.5
/// Identifies the owner of a file
pub type Uid3 = u32;
/// Group ID as defined in RFC 1813 section 2.5
/// Identifies the group ownership of a file
pub type Gid3 = u32;
/// File size in bytes as defined in RFC 1813 section 2.5
pub type Size3 = u64;
/// File offset in bytes as defined in RFC 1813 section 2.5
/// Used to specify a position within a file
pub type Offset3 = u64;
/// File mode bits as defined in RFC 1813 section 2.5
/// Contains file type and permission bits
pub type Mode3 = u32;
/// Count of bytes or entries as defined in RFC 1813 section 2.5
/// Used for various counting purposes in NFS operations
pub type Count3 = u32;

/// Used in [`NFSFh3`] to identify the file system
pub type FsId = u64;

/// Status codes returned by NFS version 3 operations
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum NFSStat3 {
    /// Indicates the call completed successfully.
    NFS3Ok = 0,
    /// Not owner. The operation was not allowed because the
    /// caller is either not a privileged user (root) or not the
    /// owner of the target of the operation.
    NFS3ErrPerm = 1,
    /// No such file or directory. The file or directory name
    /// specified does not exist.
    NFS3ErrNoEnt = 2,
    /// I/O error. A hard error (for example, a disk error)
    /// occurred while processing the requested operation.
    NFS3ErrIO = 5,
    /// I/O error. No such device or address.
    NFS3ErrNXIO = 6,
    /// Permission denied. The caller does not have the correct
    /// permission to perform the requested operation. Contrast
    /// this with `NFS3ERR_PERM`, which restricts itself to owner
    /// or privileged user permission failures.
    NFS3ErrAccess = 13,
    /// File exists. The file specified already exists.
    NFS3ErrExist = 17,
    /// Attempt to do a cross-device hard link.
    NFS3ErrXdev = 18,
    /// No such device.
    NFS3ErrNoDev = 19,
    /// Not a directory. The caller specified a non-directory in
    /// a directory operation.
    NFS3ErrNotDir = 20,
    /// Is a directory. The caller specified a directory in a
    /// non-directory operation.
    NFS3ErrIsDir = 21,
    /// Invalid argument or unsupported argument for an
    /// operation. Two examples are attempting a READLINK on an
    /// object other than a symbolic link or attempting to
    /// SETATTR a time field on a server that does not support
    /// this operation.
    NFS3ErrInval = 22,
    /// File too large. The operation would have caused a file to
    /// grow beyond the server's limit.
    NFS3ErrFBig = 27,
    /// No space left on device. The operation would have caused
    /// the server's file system to exceed its limit.
    NFS3ErrNoSpc = 28,
    /// Read-only file system. A modifying operation was
    /// attempted on a read-only file system.
    NFS3ErrROFS = 30,
    /// Too many hard links.
    NFS3ErrMLink = 31,
    /// The filename in an operation was too long.
    NFS3ErNameTooLong = 63,
    /// An attempt was made to remove a directory that was not empty.
    NFS3ErrNotEempty = 66,
    /// Resource (quota) hard limit exceeded. The user's resource
    /// limit on the server has been exceeded.
    NFS3ErrDQout = 69,
    /// Invalid file handle. The file handle given in the
    /// arguments was invalid. The file referred to by that file
    /// handle no longer exists or access to it has been
    /// revoked.
    NFS3ErrStale = 70,
    /// Too many levels of remote in path. The file handle given
    /// in the arguments referred to a file on a non-local file
    /// system on the server.
    NFS3ErrRemote = 71,
    /// Illegal NFS file handle. The file handle failed internal
    /// consistency checks.
    NFS3ErrBadHandle = 10001,
    /// Update synchronization mismatch was detected during a
    /// SETATTR operation.
    NFS3ErrNotSync = 10002,
    /// READDIR or READDIRPLUS cookie is stale
    NFS3ErrBadCookie = 10003,
    /// Operation is not supported.
    NFS3ErrNotSupp = 10004,
    /// Buffer or request is too small.
    NFS3ErrTooSmall = 10005,
    /// An error occurred on the server which does not map to any
    /// of the legal NFS version 3 protocol error values.  The
    /// client should translate this into an appropriate error.
    /// UNIX clients may choose to translate this to EIO.
    NFS3ErrServerFault = 10006,
    /// An attempt was made to create an object of a type not
    /// supported by the server.
    NFS3ErrBadType = 10007,
    /// The server initiated the request, but was not able to
    /// complete it in a timely fashion. The client should wait
    /// and then try the request with a new RPC transaction ID.
    /// For example, this error should be returned from a server
    /// that supports hierarchical storage and receives a request
    /// to process a file that has been migrated. In this case,
    /// the server should start the immigration process and
    /// respond to client with this error.
    NFS3ErrJukeBox = 10008,
}
impl SerializeEnum for NFSStat3 {}
impl DeserializeEnum for NFSStat3 {}

/// File type enumeration as defined in RFC 1813 section 2.3.5
/// Determines the type of a file system object
#[derive(Copy, Clone, Debug, Default, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum FType3 {
    /// Regular File
    #[default]
    NF3REG = 1,
    /// Directory
    NF3DIR = 2,
    /// Block Special Device
    NF3BLK = 3,
    /// Character Special Device
    NF3CHR = 4,
    /// Symbolic Link
    NF3LNK = 5,
    /// Socket
    NF3SOCK = 6,
    /// Named Pipe
    NF3FIFO = 7,
}
impl SerializeEnum for FType3 {}
impl DeserializeEnum for FType3 {}

/// Special device information for character and block special devices
/// Contains the major and minor device numbers
#[derive(Copy, Clone, Debug, Default)]
pub struct SpecData3 {
    /// Major device number
    pub specdata1: u32,
    /// Minor device number
    pub specdata2: u32,
}
DeserializeStruct!(SpecData3, specdata1, specdata2);
SerializeStruct!(SpecData3, specdata1, specdata2);

/// The NFS version 3 file handle
/// The file handle uniquely identifies a file or directory on the server
/// The server is responsible for the internal format and interpretation of the file handle
#[derive(Clone, Debug, Default)]
pub struct NFSFh3 {
    /// Used for stale handle detection
    pub gen: u64,
    /// File system identifier
    pub fs_id: FsId,
    /// Unique file identifier within the file system
    pub id: FileId3,
}
const _: () = {
    assert!(size_of::<NFSFh3>() <= NFS3_FH_SIZE as usize);
};

// Custom (de)serializer is required because in RFC NFSFh3 defined as variable-length opaque object,
// and thus needs to be encoded with its length as the first 4 bytes.
impl Deserialize for NFSFh3 {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        let len = deserialize::<UsizeAsU32>(src)?;
        if len.0 != size_of::<NFSFh3>() {
            return Err(xdr::utils::invalid_data("Invalid NFSFh3 length"));
        }
        Ok(NFSFh3 {
            gen: deserialize::<u64>(src)?,
            fs_id: deserialize::<FsId>(src)?,
            id: deserialize::<FileId3>(src)?,
        })
    }
}

impl Serialize for NFSFh3 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        UsizeAsU32(size_of::<Self>()).serialize(dest)?;
        self.gen.serialize(dest)?;
        self.fs_id.serialize(dest)?;
        self.id.serialize(dest)?;
        Ok(())
    }
}

/// NFS version 3 time structure
/// Used for file timestamps (access, modify, change)
#[derive(Copy, Clone, Debug, Default)]
pub struct NFSTime3 {
    /// Seconds since Unix epoch (January 1, 1970)
    pub seconds: u32,
    /// Nanoseconds (0-999999999)
    pub nseconds: u32,
}
DeserializeStruct!(NFSTime3, seconds, nseconds);
SerializeStruct!(NFSTime3, seconds, nseconds);

impl From<NFSTime3> for filetime::FileTime {
    fn from(time: NFSTime3) -> Self {
        Self::from_unix_time(time.seconds as i64, time.nseconds)
    }
}

/// File attributes in NFS version 3 as defined in RFC 1813 section 2.3.5
/// Contains all the standard attributes associated with a file or directory
/// in the NFS version 3 protocol
#[derive(Copy, Clone, Debug, Default)]
pub struct FAttr3 {
    /// Type of file (regular, directory, symbolic link, etc.)
    pub ftype: FType3,
    /// File access mode bits. Contains the standard Unix file
    /// permissions and file type information
    pub mode: Mode3,
    /// Number of hard links to the file. Indicates how many
    /// directory entries reference this file
    pub nlink: u32,
    /// User ID of the file owner
    pub uid: Uid3,
    /// Group ID of the file's group
    pub gid: Gid3,
    /// File size in bytes. For regular files, this is the size
    /// of the file data. For directories, this value is implementation-dependent
    pub size: Size3,
    /// Size in bytes actually allocated to the file on the server's file system
    /// This may be different from size due to block allocation policies
    pub used: Size3,
    /// Device ID information for character or block special files
    /// Contains major and minor numbers for the device
    pub rdev: SpecData3,
    /// File system identifier. Uniquely identifies the file system
    /// containing the file
    pub fsid: u64,
    /// File identifier (inode number). Uniquely identifies the file
    /// within its file system
    pub fileid: FileId3,
    /// Time of last access to the file data
    pub atime: NFSTime3,
    /// Time of last modification to the file data
    pub mtime: NFSTime3,
    /// Time of last status change (modification to the file's attributes)
    pub ctime: NFSTime3,
}
DeserializeStruct!(
    FAttr3, ftype, mode, nlink, uid, gid, size, used, rdev, fsid, fileid, atime, mtime, ctime
);
SerializeStruct!(
    FAttr3, ftype, mode, nlink, uid, gid, size, used, rdev, fsid, fileid, atime, mtime, ctime
);

/// Attributes used in weak cache consistency checking as defined in RFC 1813 section 2.3.8
/// These attributes are used to detect changes to a file by comparing
/// values before and after operations
#[derive(Copy, Clone, Debug, Default)]
pub struct WCCAttr {
    /// File size in bytes
    pub size: Size3,
    /// Last modification time of the file
    pub mtime: NFSTime3,
    /// Last status change time of the file
    pub ctime: NFSTime3,
}
DeserializeStruct!(WCCAttr, size, mtime, ctime);
SerializeStruct!(WCCAttr, size, mtime, ctime);

impl From<FAttr3> for WCCAttr {
    fn from(attr: FAttr3) -> Self {
        WCCAttr { size: attr.size, mtime: attr.mtime, ctime: attr.ctime }
    }
}

/// Pre-operation attributes for weak cache consistency as defined in RFC 1813 section 2.3.8
/// These attributes represent the file state before an operation was performed
/// Used together with post-operation attributes to determine if file state changed
pub type PreOpAttr = Option<WCCAttr>;

/// Post-operation attributes for file information as defined in RFC 1813 section 2.3.8
/// These attributes represent the file state after an operation was performed
/// Returned in almost all NFS procedure responses to allow clients to maintain
/// a consistent cache of file attributes
pub type PostOpAttr = Option<FAttr3>;

/// Weak cache consistency data as defined in RFC 1813 section 2.3.8
/// Contains file attributes before and after an operation
/// This data structure is returned by operations that modify file attributes
/// to allow clients to update their cached attributes appropriately
#[derive(Copy, Clone, Debug, Default)]
pub struct WCCData {
    /// File attributes before operation
    pub before: PreOpAttr,
    /// File attributes after operation
    pub after: PostOpAttr,
}
DeserializeStruct!(WCCData, before, after);
SerializeStruct!(WCCData, before, after);

pub type PostOpFh3 = Option<NFSFh3>;
pub type SetMode3 = Option<Mode3>;
pub type SetUid3 = Option<Uid3>;
pub type SetGid3 = Option<Gid3>;
pub type SetSize3 = Option<Size3>;

/// Specifies how to modify the last access time (atime) during a `SETATTR` operation.
/// This enum allows the client to either:
/// - Leave the atime unchanged (`DONT_CHANGE`)
/// - Set it to the server's current time (`SET_TO_SERVER_TIME`)
/// - Set it to a specific client-provided time (`SET_TO_CLIENT_TIME`)
#[derive(Copy, Clone, Debug)]
#[repr(u32)]
pub enum SetAtime {
    /// Don't modify the file's last access time
    DontChange,
    /// Set the file's last access time to the server's current time
    SetToServerTime,
    /// Set the file's last access time to the specified time value
    SetToClientTime(NFSTime3),
}

impl Serialize for SetAtime {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            SetAtime::DontChange => {
                0_u32.serialize(dest)?;
            }
            SetAtime::SetToServerTime => {
                1_u32.serialize(dest)?;
            }
            SetAtime::SetToClientTime(v) => {
                2_u32.serialize(dest)?;
                v.serialize(dest)?;
            }
        }

        Ok(())
    }
}
impl Deserialize for SetAtime {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        match deserialize::<u32>(src)? {
            0 => Ok(SetAtime::DontChange),
            1 => Ok(SetAtime::SetToServerTime),
            2 => Ok(SetAtime::SetToClientTime(deserialize(src)?)),
            c => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid set_atime value: {c}"),
            )),
        }
    }
}

/// Specifies how to modify the last modification time (mtime) during a `SETATTR` operation.
/// This enum allows the client to either:
/// - Leave the mtime unchanged
/// - Set it to the server's current time
/// - Set it to a specific client-provided time
///
/// The discriminant value follows the `time_how` enumeration from RFC 1813
#[derive(Copy, Clone, Debug)]
#[repr(u32)]
pub enum SetMtime {
    /// Keep the current modification time unchanged
    DontChange,
    /// Update the modification time to the server's current time
    SetToServerTime,
    /// Set the modification time to a specific timestamp provided by the client
    SetToClientTime(NFSTime3),
}

impl Serialize for SetMtime {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        match self {
            SetMtime::DontChange => {
                0_u32.serialize(dest)?;
            }
            SetMtime::SetToServerTime => {
                1_u32.serialize(dest)?;
            }
            SetMtime::SetToClientTime(v) => {
                2_u32.serialize(dest)?;
                v.serialize(dest)?;
            }
        }

        Ok(())
    }
}
impl Deserialize for SetMtime {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        match deserialize::<u32>(src)? {
            0 => Ok(SetMtime::DontChange),
            1 => Ok(SetMtime::SetToServerTime),
            2 => Ok(SetMtime::SetToClientTime(deserialize(src)?)),
            c => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid set_mtime value: {c}"),
            )),
        }
    }
}

/// Set of file attributes to change in `SETATTR` operations
#[derive(Copy, Clone, Debug)]
pub struct SAttr3 {
    /// File mode (permissions)
    pub mode: SetMode3,
    /// User ID of owner
    pub uid: SetUid3,
    /// Group ID of owner
    pub gid: SetGid3,
    /// File size
    pub size: SetSize3,
    /// Last access time
    pub atime: SetAtime,
    /// Last modification time
    pub mtime: SetMtime,
}
DeserializeStruct!(SAttr3, mode, uid, gid, size, atime, mtime);
SerializeStruct!(SAttr3, mode, uid, gid, size, atime, mtime);

impl Default for SAttr3 {
    fn default() -> SAttr3 {
        SAttr3 {
            mode: SetMode3::None,
            uid: SetUid3::None,
            gid: SetGid3::None,
            size: SetSize3::None,
            atime: SetAtime::DontChange,
            mtime: SetMtime::DontChange,
        }
    }
}

/// Arguments for directory operations (specifying directory handle and name)
#[derive(Clone, Debug, Default)]
pub struct DiropArgs3 {
    /// Directory file handle
    pub dir: NFSFh3,
    /// Name within the directory
    pub name: Filename3,
}
DeserializeStruct!(DiropArgs3, dir, name);
SerializeStruct!(DiropArgs3, dir, name);

/// Data for creating a symbolic link
#[derive(Debug, Default)]
pub struct SymlinkData3 {
    /// Attributes for the symbolic link
    pub symlink_attributes: SAttr3,
    /// Target path for the symbolic link
    pub symlink_data: NFSPath3,
}
DeserializeStruct!(SymlinkData3, symlink_attributes, symlink_data);
SerializeStruct!(SymlinkData3, symlink_attributes, symlink_data);

/// Gets the root file handle for mounting
pub fn get_root_mount_handle() -> Vec<u8> {
    vec![0]
}

/// Access permission to read file data or read a directory as defined in RFC 1813 section 3.3.4
pub const ACCESS3_READ: u32 = 0x0001;
/// Access permission to look up names in a directory as defined in RFC 1813 section 3.3.4
pub const ACCESS3_LOOKUP: u32 = 0x0002;
/// Access permission to modify the contents of an existing file as defined in RFC 1813 section 3.3.4
pub const ACCESS3_MODIFY: u32 = 0x0004;
/// Access permission to grow the file's size or extend a directory by adding entries
/// as defined in RFC 1813 section 3.3.4
pub const ACCESS3_EXTEND: u32 = 0x0008;
/// Access permission to delete a file or directory entry as defined in RFC 1813 section 3.3.4
pub const ACCESS3_DELETE: u32 = 0x0010;
/// Access permission to execute a file or traverse a directory as defined in RFC 1813 section 3.3.4
pub const ACCESS3_EXECUTE: u32 = 0x0020;

/// File creation modes for `CREATE` operations
#[derive(Copy, Clone, Debug, Default, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum CreateMode3 {
    /// Normal file creation - doesn't error if file exists
    #[default]
    Unchecked = 0,
    /// Return error if file exists
    Guarded = 1,
    /// Use exclusive create mechanism (with verifier)
    Exclusive = 2,
}
impl SerializeEnum for CreateMode3 {}
impl DeserializeEnum for CreateMode3 {}

pub type SAttrGuard3 = Option<NFSTime3>;

/// Arguments for `SETATTR` operations
#[derive(Clone, Debug, Default)]
pub struct SETATTR3args {
    /// File handle for target file
    pub object: NFSFh3,
    /// New attributes to set
    pub new_attribute: SAttr3,
    /// Guard condition for atomic change
    pub guard: Option<NFSTime3>,
}
DeserializeStruct!(SETATTR3args, object, new_attribute, guard);
SerializeStruct!(SETATTR3args, object, new_attribute, guard);
