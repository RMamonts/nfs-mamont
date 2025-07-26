//! Module contains XDR data structures related to file operations for NFS version 3 protocol
//! as defined in RFC 1813.
//!
//! This module includes data structures for the following operations:
//! - GETATTR: Get file attributes (procedure 1)
//! - SETATTR: Set file attributes (procedure 2)
//! - LOOKUP: Lookup filename (procedure 3)
//! - ACCESS: Check access permission (procedure 4)
//! - READLINK: Read from symbolic link (procedure 5)
//! - READ: Read data from a file (procedure 6)
//! - WRITE: Write data to a file (procedure 7)
//! - CREATE: Create a file (procedure 8)
//! - REMOVE: Remove a file (procedure 12)
//! - RENAME: Rename a file or directory (procedure 14)
//! - LINK: Create a hard link (procedure 15)
//! - COMMIT: Commit asynchronously written data to stable storage (procedure 21)
//!
//! The structures implement the XDR serialization/deserialization interfaces for
//! the request arguments and response data of these operations.

// Allow unused code warnings since we implement the complete RFC 1813 specification,
// including procedures that may not be used by all clients
#![allow(dead_code)]
// Preserve original RFC naming conventions (e.g. READ3args, COMMIT3resok)
// for consistency with the NFS version 3 protocol specification
#![allow(non_camel_case_types)]

use std::io::{Read, Write};

use num_derive::{FromPrimitive, ToPrimitive};

use super::{
    count3, diropargs3, fattr3, nfs_fh3, offset3, post_op_attr, post_op_fh3, sattr3, wcc_data,
    writeverf3, DeserializeEnum, DeserializeStruct, SerializeEnum,
    SerializeStruct,
};
use crate::xdr::nfs3::{nfspath3, sattrguard3};

/// Arguments for the GETATTR procedure (procedure 1) as defined in RFC 1813 section 3.3.1
/// Used to retrieve the attributes for a specified file system object
#[derive(Debug, Default)]
pub struct GETATTR3args {
    /// File handle of an object whose attributes are to be retrieved
    pub object: nfs_fh3,
}
DeserializeStruct!(GETATTR3args, object);
SerializeStruct!(GETATTR3args, object);

/// Successful response for the GETATTR procedure as defined in RFC 1813 section 3.3.1
#[derive(Debug, Default)]
pub struct GETATTR3resok {
    /// The attributes for the object
    pub obj_attributes: fattr3,
}
DeserializeStruct!(GETATTR3resok, obj_attributes);
SerializeStruct!(GETATTR3resok, obj_attributes);

/// Failed response for the GETATTR procedure as defined in RFC 1813 section 3.3.1
#[derive(Debug, Default)]
pub struct GETATTR3resfail {}
DeserializeStruct!(GETATTR3resfail,);
SerializeStruct!(GETATTR3resfail,);

/// Arguments for the SETATTR procedure (procedure 2) as defined in RFC 1813 section 3.3.2
/// Used to set the attributes of a file system object
#[derive(Debug, Default)]
pub struct SETATTR3args {
    /// File handle for the object whose attributes are to be set
    pub object: nfs_fh3,
    /// New attributes to be set on the object
    pub new_attributes: sattr3,
    /// Guard to provide conditional protection for the operation
    pub guard: sattrguard3,
}
DeserializeStruct!(SETATTR3args, object, new_attributes, guard);
SerializeStruct!(SETATTR3args, object, new_attributes, guard);

/// Successful response for the SETATTR procedure as defined in RFC 1813 section 3.3.2
#[derive(Debug, Default)]
pub struct SETATTR3resok {
    /// A wcc_data structure containing the old and new attributes for the object.
    pub obj_wcc: wcc_data,
}
DeserializeStruct!(SETATTR3resok, obj_wcc);
SerializeStruct!(SETATTR3resok, obj_wcc);

/// Failed response for the SETATTR procedure as defined in RFC 1813 section 3.3.2
#[derive(Debug, Default)]
pub struct SETATTR3resfail {
    /// A wcc_data structure containing the old and new attributes for the object.
    pub obj_wcc: wcc_data,
}
DeserializeStruct!(SETATTR3resfail, obj_wcc);
SerializeStruct!(SETATTR3resfail, obj_wcc);

/// Arguments for the LOOKUP procedure (procedure 3) as defined in RFC 1813 section 3.3.3
/// Used to search a directory for a specific name and return the file handle
#[derive(Debug, Default)]
pub struct LOOKUP3args {
    /// Object to look up containing directory handle and filename
    pub what: diropargs3,
}
DeserializeStruct!(LOOKUP3args, what);
SerializeStruct!(LOOKUP3args, what);

/// Successful response for the LOOKUP procedure as defined in RFC 1813 section 3.3.3
#[derive(Debug, Default)]
pub struct LOOKUP3resok {
    /// File handle of the object corresponding to the looked up name
    pub object: nfs_fh3,
    /// Post-operation attributes of the object
    pub obj_attributes: post_op_attr,
    /// Post-operation attributes of the directory
    pub dir_attributes: post_op_attr,
}
DeserializeStruct!(LOOKUP3resok, object, obj_attributes, dir_attributes);
SerializeStruct!(LOOKUP3resok, object, obj_attributes, dir_attributes);

/// Failed response for the LOOKUP procedure as defined in RFC 1813 section 3.3.3
#[derive(Debug, Default)]
pub struct LOOKUP3resfail {
    /// Post-operation attributes of the directory
    pub dir_attributes: post_op_attr,
}
DeserializeStruct!(LOOKUP3resfail, dir_attributes);
SerializeStruct!(LOOKUP3resfail, dir_attributes);

/// Arguments for the ACCESS procedure (procedure 4) as defined in RFC 1813 section 3.3.4
/// Used to determine access rights for a file system object
#[derive(Debug, Default)]
pub struct ACCESS3args {
    /// File handle for the file system object to check access
    pub object: nfs_fh3,
    /// Bit mask of access permissions to check
    pub access: u32,
}
DeserializeStruct!(ACCESS3args, object, access);
SerializeStruct!(ACCESS3args, object, access);

/// Successful response for the ACCESS procedure as defined in RFC 1813 section 3.3.4
#[derive(Debug, Default)]
pub struct ACCESS3resok {
    /// Post-operation attributes of the object
    pub obj_attributes: post_op_attr,
    /// Bit mask indicating allowed access permissions
    pub access: u32,
}
DeserializeStruct!(ACCESS3resok, obj_attributes, access);
SerializeStruct!(ACCESS3resok, obj_attributes, access);

/// Failed response for the ACCESS procedure as defined in RFC 1813 section 3.3.4
#[derive(Debug, Default)]
pub struct ACCESS3resfail {
    /// Attributes of the object if access to attributes is permitted
    pub obj_attributes: post_op_attr,
}
DeserializeStruct!(ACCESS3resfail, obj_attributes);
SerializeStruct!(ACCESS3resfail, obj_attributes);

/// Arguments for the READLINK procedure (procedure 5) as defined in RFC 1813 section 3.3.5
/// Used to read the data associated with a symbolic link
#[derive(Debug, Default)]
pub struct READLINK3args {
    /// File handle for a symbolic link (file system object of type NF3LNK)
    pub symlink: nfs_fh3,
}
DeserializeStruct!(READLINK3args, symlink);
SerializeStruct!(READLINK3args, symlink);

/// Successful response for the READLINK procedure as defined in RFC 1813 section 3.3.5
#[derive(Debug, Default)]
pub struct READLINK3resok {
    /// Post-operation attributes for the symbolic link
    pub symlink_attributes: post_op_attr,
    /// The data associated with the symbolic link
    pub data: nfspath3,
}
DeserializeStruct!(READLINK3resok, symlink_attributes, data);
SerializeStruct!(READLINK3resok, symlink_attributes, data);

/// Failed response for the READLINK procedure as defined in RFC 1813 section 3.3.5
#[derive(Debug, Default)]
pub struct READLINK3resfail {
    /// Post-operation attributes for the symbolic link
    pub symlink_attributes: post_op_attr,
}
DeserializeStruct!(READLINK3resfail, symlink_attributes);
SerializeStruct!(READLINK3resfail, symlink_attributes);

/// Arguments for the READ procedure (procedure 6) as defined in RFC 1813 section 3.3.6
/// Used to read data from a regular file
#[derive(Debug, Default)]
pub struct READ3args {
    /// File handle for the file to be read
    pub file: nfs_fh3,
    /// Position within the file to begin reading
    pub offset: offset3,
    /// Number of bytes of data to read
    pub count: count3,
}
DeserializeStruct!(READ3args, file, offset, count);
SerializeStruct!(READ3args, file, offset, count);

/// Successful response for the READ procedure as defined in RFC 1813 section 3.3.6
#[derive(Debug, Default)]
pub struct READ3resok {
    /// File attributes after the operation
    pub file_attributes: post_op_attr,
    /// Number of bytes actually read
    pub count: count3,
    /// True if the end of file was reached
    pub eof: bool,
    /// The data read from the file
    pub data: Vec<u8>,
}
DeserializeStruct!(READ3resok, file_attributes, count, eof, data);
SerializeStruct!(READ3resok, file_attributes, count, eof, data);

/// Failed response for the READ procedure as defined in RFC 1813 section 3.3.6
#[derive(Debug, Default)]
pub struct READ3resfail {
    /// Post-operation attributes of the file
    pub file_attributes: post_op_attr,
}
DeserializeStruct!(READ3resfail, file_attributes);
SerializeStruct!(READ3resfail, file_attributes);

/// Enumeration specifying how data should be written to storage
/// as defined in RFC 1813 section 3.3.7
#[derive(Copy, Clone, Debug, Default, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum stable_how {
    /// Data may be buffered before writing to stable storage
    /// The server may return before the data is committed to stable storage
    #[default]
    UNSTABLE = 0,
    /// Data must be committed to stable storage before returning
    /// Only the data for this request is guaranteed to be committed
    DATA_SYNC = 1,
    /// All file system data must be committed to stable storage before returning
    /// This includes the data and all metadata for this request
    FILE_SYNC = 2,
}
impl SerializeEnum for stable_how {}
impl DeserializeEnum for stable_how {}

/// Arguments for the WRITE procedure (procedure 7) as defined in RFC 1813 section 3.3.7
/// Used to write data to a regular file
#[derive(Debug, Default)]
pub struct WRITE3args {
    /// File handle for the file to write
    pub file: nfs_fh3,
    /// Position within the file to begin writing
    pub offset: offset3,
    /// Number of bytes of data to write
    pub count: count3,
    /// How to commit the data to storage
    pub stable: stable_how,
    /// The data to be written
    pub data: Vec<u8>,
}
DeserializeStruct!(WRITE3args, file, offset, count, stable, data);
SerializeStruct!(WRITE3args, file, offset, count, stable, data);

/// Successful response for the WRITE procedure as defined in RFC 1813 section 3.3.7
#[derive(Debug, Default)]
pub struct WRITE3resok {
    /// File attributes before and after the operation
    pub file_wcc: wcc_data,
    /// Number of bytes actually written
    pub count: count3,
    /// How the data was committed to stable storage
    pub committed: stable_how,
    /// Write verifier to detect server restarts
    pub verf: writeverf3,
}
DeserializeStruct!(WRITE3resok, file_wcc, count, committed, verf);
SerializeStruct!(WRITE3resok, file_wcc, count, committed, verf);

/// Failed response for the WRITE procedure as defined in RFC 1813 section 3.3.7
#[derive(Debug, Default)]
pub struct WRITE3resfail {
    /// Weak cache consistency data for the file
    pub file_wcc: wcc_data,
}
DeserializeStruct!(WRITE3resfail, file_wcc);
SerializeStruct!(WRITE3resfail, file_wcc);

/// File creation modes for `CREATE` operations
// TODO: createhow3
#[derive(Copy, Clone, Debug, Default, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum createmode3 {
    /// Normal file creation - doesn't error if file exists
    #[default]
    UNCHECKED = 0,
    /// Return error if file exists
    GUARDED = 1,
    /// Use exclusive create mechanism (with verifier)
    EXCLUSIVE = 2,
}
impl SerializeEnum for createmode3 {}
impl DeserializeEnum for createmode3 {}

/// Arguments for the CREATE procedure (procedure 8) as defined in RFC 1813 section 3.3.8
/// Used to create a regular file
#[derive(Debug, Default)]
pub struct CREATE3args {
    /// Location and name of the file to be created
    pub where_: diropargs3,
    /// Discriminated union describing how the server should handle file creation
    pub how: createmode3,
}
DeserializeStruct!(CREATE3args, where_, how);
SerializeStruct!(CREATE3args, where_, how);

/// Successful response for the CREATE procedure as defined in RFC 1813 section 3.3.8
#[derive(Debug, Default)]
pub struct CREATE3resok {
    /// File handle of the newly created regular file
    pub obj: post_op_fh3,
    /// Attributes of the regular file just created
    pub obj_attributes: post_op_attr,
    /// Weak cache consistency data for the directory
    pub dir_wcc: wcc_data,
}
DeserializeStruct!(CREATE3resok, obj, obj_attributes, dir_wcc);
SerializeStruct!(CREATE3resok, obj, obj_attributes, dir_wcc);

/// Failed response for the CREATE procedure as defined in RFC 1813 section 3.3.8
#[derive(Debug, Default)]
pub struct CREATE3resfail {
    /// Weak cache consistency data for the directory
    pub dir_wcc: wcc_data,
}
DeserializeStruct!(CREATE3resfail, dir_wcc);
SerializeStruct!(CREATE3resfail, dir_wcc);

/// Arguments for the REMOVE procedure (procedure 12) as defined in RFC 1813 section 3.3.12
/// Used to remove (delete) an entry from a directory
#[derive(Debug, Default)]
pub struct REMOVE3args {
    /// Diropargs3 structure identifying the entry to be removed
    pub object: diropargs3,
}
DeserializeStruct!(REMOVE3args, object);
SerializeStruct!(REMOVE3args, object);

/// Successful response for the REMOVE procedure as defined in RFC 1813 section 3.3.12
#[derive(Debug, Default)]
pub struct REMOVE3resok {
    /// Weak cache consistency data for the directory
    pub dir_wcc: wcc_data,
}
DeserializeStruct!(REMOVE3resok, dir_wcc);
SerializeStruct!(REMOVE3resok, dir_wcc);

/// Failed response for the REMOVE procedure as defined in RFC 1813 section 3.3.12
#[derive(Debug, Default)]
pub struct REMOVE3resfail {
    /// Weak cache consistency data for the directory
    pub dir_wcc: wcc_data,
}
DeserializeStruct!(REMOVE3resfail, dir_wcc);
SerializeStruct!(REMOVE3resfail, dir_wcc);

/// Arguments for the RENAME procedure (procedure 14) as defined in RFC 1813 section 3.3.14
/// Used to rename a file or directory
#[derive(Debug, Default)]
pub struct RENAME3args {
    /// Source object to be renamed
    pub from: diropargs3,
    /// Destination name and location
    pub to: diropargs3,
}
DeserializeStruct!(RENAME3args, from, to);
SerializeStruct!(RENAME3args, from, to);

/// Successful response for the RENAME procedure as defined in RFC 1813 section 3.3.14
#[derive(Debug, Default)]
pub struct RENAME3resok {
    /// Weak cache consistency data for the source directory
    pub fromdir_wcc: wcc_data,
    /// Weak cache consistency data for the destination directory
    pub todir_wcc: wcc_data,
}
DeserializeStruct!(RENAME3resok, fromdir_wcc, todir_wcc);
SerializeStruct!(RENAME3resok, fromdir_wcc, todir_wcc);

/// Failed response for the RENAME procedure as defined in RFC 1813 section 3.3.14
#[derive(Debug, Default)]
pub struct RENAME3resfail {
    /// Weak cache consistency data for the source directory
    pub fromdir_wcc: wcc_data,
    /// Weak cache consistency data for the destination directory
    pub todir_wcc: wcc_data,
}
DeserializeStruct!(RENAME3resfail, fromdir_wcc, todir_wcc);
SerializeStruct!(RENAME3resfail, fromdir_wcc, todir_wcc);

/// Arguments for the LINK procedure (procedure 15) as defined in RFC 1813 section 3.3.15
/// Used to create a hard link to a file
#[derive(Debug, Default)]
pub struct LINK3args {
    /// File handle for the target file
    pub file: nfs_fh3,
    /// Directory and name for the new link
    pub link: diropargs3,
}
DeserializeStruct!(LINK3args, file, link);
SerializeStruct!(LINK3args, file, link);

/// Successful response for the LINK procedure as defined in RFC 1813 section 3.3.15
#[derive(Debug, Default)]
pub struct LINK3resok {
    /// Post-operation attributes of the file system object
    pub file_attributes: post_op_attr,
    /// Weak cache consistency data for the directory
    pub linkdir_wcc: wcc_data,
}
DeserializeStruct!(LINK3resok, file_attributes, linkdir_wcc);
SerializeStruct!(LINK3resok, file_attributes, linkdir_wcc);

/// Failed response for the LINK procedure as defined in RFC 1813 section 3.3.15
#[derive(Debug, Default)]
pub struct LINK3resfail {
    /// Post-operation attributes of the file system object
    pub file_attributes: post_op_attr,
    /// Weak cache consistency data for the directory
    pub linkdir_wcc: wcc_data,
}
DeserializeStruct!(LINK3resfail, file_attributes, linkdir_wcc);
SerializeStruct!(LINK3resfail, file_attributes, linkdir_wcc);

/// Arguments for the COMMIT procedure (procedure 21) as defined in RFC 1813 section 3.3.21
/// Used to commit pending writes to stable storage
#[derive(Debug, Default)]
pub struct COMMIT3args {
    /// File handle for the file to commit
    pub file: nfs_fh3,
    /// Position within the file to start committing
    pub offset: offset3,
    /// Number of bytes to commit
    pub count: count3,
}
DeserializeStruct!(COMMIT3args, file, offset, count);
SerializeStruct!(COMMIT3args, file, offset, count);

/// Successful response for the COMMIT procedure as defined in RFC 1813 section 3.3.21
#[derive(Debug, Default)]
pub struct COMMIT3resok {
    /// File attributes before and after the operation
    pub file_wcc: wcc_data,
    /// Write verifier to detect server restarts
    pub verf: writeverf3,
}
DeserializeStruct!(COMMIT3resok, file_wcc, verf);
SerializeStruct!(COMMIT3resok, file_wcc, verf);

/// Failed response for the COMMIT procedure as defined in RFC 1813 section 3.3.21
#[derive(Debug, Default)]
pub struct COMMIT3resfail {
    /// Weak cache consistency data for the file
    pub file_wcc: wcc_data,
}
DeserializeStruct!(COMMIT3resfail, file_wcc);
SerializeStruct!(COMMIT3resfail, file_wcc);
