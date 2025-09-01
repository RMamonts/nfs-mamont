//! Module contains XDR data structures related to directories for NFS version 3 protocol
//! as defined in RFC 1813.
//!
//! This module includes data structures for the following directory operations:
//! - MKDIR: Create a directory (procedure 9)
//! - SYMLINK: Create a symbolic link (procedure 10)
//! - RMDIR: Remove a directory (procedure 13)
//! - READDIR: Read from a directory (procedure 16)
//! - READDIRPLUS: Extended read from a directory (procedure 17)
//!
//! These structures implement the XDR serialization/deserialization interfaces for
//! the request arguments and response data of directory-related operations.

// Allow unused code warnings since we implement the complete RFC 1813 specification,
// including procedures that may not be used by all clients
#![allow(dead_code)]
// Preserve original RFC naming conventions (e.g. READDIR3args, MKDIR3resok)
// for consistency with the NFS version 3 protocol specification
#![allow(non_camel_case_types)]

use std::io::{Read, Write};

use super::{
    cookie3, cookieverf3, count3, diropargs3, fileid3, filename3, nfs_fh3, post_op_attr,
    post_op_fh3, sattr3, DeserializeStruct, SerializeStruct,
};
use crate::xdr::nfs3::{symlinkdata3, wcc_data};

/// Arguments for the MKDIR procedure (procedure 9) as defined in RFC 1813 section 3.3.9
/// Used to create a new directory
#[derive(Debug, Default)]
pub struct MKDIR3args {
    /// Directory where new directory should be created and its name
    pub where_: diropargs3,
    /// Initial attributes for the new directory
    pub attributes: sattr3,
}
DeserializeStruct!(MKDIR3args, where_, attributes);
SerializeStruct!(MKDIR3args, where_, attributes);

/// Successful response for the MKDIR procedure as defined in RFC 1813 section 3.3.9
#[derive(Debug, Default)]
pub struct MKDIR3resok {
    /// File handle for the newly created directory
    pub obj: post_op_fh3,
    /// Attributes for the newly created subdirectory
    pub obj_attributes: post_op_attr,
    /// Weak cache consistency data for the directory
    pub dir_wcc: wcc_data,
}
DeserializeStruct!(MKDIR3resok, obj, obj_attributes, dir_wcc);
SerializeStruct!(MKDIR3resok, obj, obj_attributes, dir_wcc);

/// Failed response for the MKDIR procedure as defined in RFC 1813 section 3.3.9
#[derive(Debug, Default)]
pub struct MKDIR3resfail {
    /// Weak cache consistency data for the directory
    pub dir_wcc: wcc_data,
}
DeserializeStruct!(MKDIR3resfail, dir_wcc);
SerializeStruct!(MKDIR3resfail, dir_wcc);

/// Arguments for the SYMLINK procedure (procedure 10) as defined in RFC 1813 section 3.3.10
/// Used to create a symbolic link
#[derive(Debug, Default)]
pub struct SYMLINK3args {
    /// Directory where symbolic link should be created and its name
    pub where_: diropargs3,
    /// Target path and attributes for the symbolic link
    pub symlink: symlinkdata3,
}
DeserializeStruct!(SYMLINK3args, where_, symlink);
SerializeStruct!(SYMLINK3args, where_, symlink);

/// Successful response for the SYMLINK procedure as defined in RFC 1813 section 3.3.10
#[derive(Debug, Default)]
pub struct SYMLINK3resok {
    /// File handle for the newly created symbolic link
    pub obj: post_op_fh3,
    /// Attributes for the newly created symbolic link
    pub obj_attributes: post_op_attr,
    /// Weak cache consistency data for the directory
    pub dir_wcc: wcc_data,
}
DeserializeStruct!(SYMLINK3resok, obj, obj_attributes, dir_wcc);
SerializeStruct!(SYMLINK3resok, obj, obj_attributes, dir_wcc);

/// Failed response for the SYMLINK procedure as defined in RFC 1813 section 3.3.10
#[derive(Debug, Default)]
pub struct SYMLINK3resfail {
    /// Weak cache consistency data for the directory
    pub dir_wcc: wcc_data,
}
DeserializeStruct!(SYMLINK3resfail, dir_wcc);
SerializeStruct!(SYMLINK3resfail, dir_wcc);

/// Arguments for the RMDIR procedure (procedure 13) as defined in RFC 1813 section 3.3.13
/// Used to remove (delete) a subdirectory from a directory
#[derive(Debug, Default)]
pub struct RMDIR3args {
    /// Diropargs3 structure identifying the directory entry to be removed
    pub object: diropargs3,
}
DeserializeStruct!(RMDIR3args, object);
SerializeStruct!(RMDIR3args, object);

/// Successful response for the RMDIR procedure as defined in RFC 1813 section 3.3.13
#[derive(Debug, Default)]
pub struct RMDIR3resok {
    /// Weak cache consistency data for the directory
    pub dir_wcc: wcc_data,
}
DeserializeStruct!(RMDIR3resok, dir_wcc);
SerializeStruct!(RMDIR3resok, dir_wcc);

/// Failed response for the RMDIR procedure as defined in RFC 1813 section 3.3.13
#[derive(Debug, Default)]
pub struct RMDIR3resfail {
    /// Weak cache consistency data for the directory
    pub dir_wcc: wcc_data,
}
DeserializeStruct!(RMDIR3resfail, dir_wcc);
SerializeStruct!(RMDIR3resfail, dir_wcc);

/// Arguments for the READDIR procedure (procedure 16) as defined in RFC 1813 section 3.3.16
/// Used to read entries from a directory. The server returns a variable number of directory entries,
/// up to the specified count limit.
#[derive(Debug, Default)]
pub struct READDIR3args {
    /// File handle for the directory to be read
    pub dir: nfs_fh3,
    /// Cookie indicating where to start reading directory entries
    /// A cookie value of 0 means start at beginning of directory
    pub cookie: cookie3,
    /// Cookie verifier to detect whether directory has changed
    pub cookieverf: cookieverf3,
    /// Maximum number of bytes of directory information to return
    pub count: count3,
}
DeserializeStruct!(READDIR3args, dir, cookie, cookieverf, count);
SerializeStruct!(READDIR3args, dir, cookie, cookieverf, count);

/// Directory entry returned by READDIR operation as defined in RFC 1813 section 3.3.16
#[derive(Debug, Default, Clone)]
pub struct entry3 {
    /// File identifier (inode number)
    pub fileid: fileid3,
    /// Name of the directory entry
    pub name: filename3,
    /// Cookie for the next READDIR operation
    pub cookie: cookie3,
}
DeserializeStruct!(entry3, fileid, name, cookie);
SerializeStruct!(entry3, fileid, name, cookie);

/// Directory list returned by READDIR operation as defined in RFC 1813 section 3.3.16
#[derive(Debug, Default)]
pub struct dirlist3 {
    /// Zero or more directory entries
    pub entries: Vec<entry3>,
    /// TRUE if the last entry is the last entry in the directory
    pub eof: bool,
}
DeserializeStruct!(dirlist3, entries, eof);
SerializeStruct!(dirlist3, entries, eof);

/// Successful response for the READDIR procedure as defined in RFC 1813 section 3.3.16
#[derive(Debug, Default)]
pub struct READDIR3resok {
    /// Attributes of the directory
    pub dir_attributes: post_op_attr,
    /// Cookie verifier
    pub cookieverf: cookieverf3,
    /// Directory list
    pub reply: dirlist3,
}
DeserializeStruct!(READDIR3resok, dir_attributes, cookieverf, reply);
SerializeStruct!(READDIR3resok, dir_attributes, cookieverf, reply);

/// Failed response for the READDIR procedure as defined in RFC 1813 section 3.3.16
#[derive(Debug, Default)]
pub struct READDIR3resfail {
    /// Attributes of the directory
    pub dir_attributes: post_op_attr,
}
DeserializeStruct!(READDIR3resfail, dir_attributes);
SerializeStruct!(READDIR3resfail, dir_attributes);

/// Arguments for the READDIRPLUS procedure (procedure 17) as defined in RFC 1813 section 3.3.17
/// READDIRPLUS returns directory entries along with their attributes and file handles.
#[derive(Debug, Default)]
pub struct READDIRPLUS3args {
    /// Directory file handle
    pub dir: nfs_fh3,
    /// Cookie from previous READDIRPLUS - where to start reading
    pub cookie: cookie3,
    /// Cookie verifier to detect changed directories
    pub cookieverf: cookieverf3,
    /// Maximum number of bytes of directory information to return
    pub dircount: count3,
    /// Maximum number of bytes of attribute information to return
    pub maxcount: count3,
}
DeserializeStruct!(READDIRPLUS3args, dir, cookie, cookieverf, dircount, maxcount);
SerializeStruct!(READDIRPLUS3args, dir, cookie, cookieverf, dircount, maxcount);

/// Directory entry with additional attributes for READDIRPLUS operation as defined in RFC 1813 section 3.3.17
/// This structure represents a single directory entry with extended information
#[derive(Debug, Default, Clone)]
pub struct entryplus3 {
    /// File identifier (inode number) uniquely identifying the file within the filesystem
    pub fileid: fileid3,
    /// Name of the directory entry (filename)
    pub name: filename3,
    /// Cookie value that can be used in subsequent READDIRPLUS calls to resume listing
    pub cookie: cookie3,
    /// File attributes for this directory entry
    pub name_attributes: post_op_attr,
    /// File handle for this directory entry
    pub name_handle: post_op_fh3,
}
DeserializeStruct!(entryplus3, fileid, name, cookie, name_attributes, name_handle);
SerializeStruct!(entryplus3, fileid, name, cookie, name_attributes, name_handle);

/// Directory list with attributes returned by READDIRPLUS operation as defined in RFC 1813 section 3.3.17
#[derive(Debug, Default)]
pub struct dirlistplus3 {
    /// Zero or more directory entries with attributes and file handles
    pub entries: Vec<entryplus3>,
    /// TRUE if the last entry is the last entry in the directory
    pub eof: bool,
}
DeserializeStruct!(dirlistplus3, entries, eof);
SerializeStruct!(dirlistplus3, entries, eof);

/// Successful response for the READDIRPLUS procedure as defined in RFC 1813 section 3.3.17
#[derive(Debug, Default)]
pub struct READDIRPLUS3resok {
    /// Attributes of the directory
    pub dir_attributes: post_op_attr,
    /// Cookie verifier
    pub cookieverf: cookieverf3,
    /// Directory list with attributes
    pub reply: dirlistplus3,
}
DeserializeStruct!(READDIRPLUS3resok, dir_attributes, cookieverf, reply);
SerializeStruct!(READDIRPLUS3resok, dir_attributes, cookieverf, reply);

/// Failed response for the READDIRPLUS procedure as defined in RFC 1813 section 3.3.17
#[derive(Debug, Default)]
pub struct READDIRPLUS3resfail {
    /// Attributes of the directory
    pub dir_attributes: post_op_attr,
}
DeserializeStruct!(READDIRPLUS3resfail, dir_attributes);
SerializeStruct!(READDIRPLUS3resfail, dir_attributes);
