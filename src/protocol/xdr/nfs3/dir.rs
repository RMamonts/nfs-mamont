//! Module contains XDR data structures related to directories for NFS version 3 protocol
//! as defined in RFC 1813.
//!
//! This module includes data structures for the following directory operations:
//! - MKDIR: Create a directory (procedure 9)
//! - READDIR: Read from a directory (procedure 16)
//! - READDIRPLUS: Extended read from a directory (procedure 17)
//!
//! These structures implement the XDR serialization/deserialization interfaces for
//! the request arguments and response data of directory-related operations.

use std::io::{Read, Write};

use super::{
    cookie3, cookieverf3, count3, diropargs3, fileid3, filename3, nfs_fh3, post_op_attr,
    post_op_fh3, sattr3, Deserialize, DeserializeStruct, Serialize, SerializeStruct,
};

/// Arguments for the MKDIR procedure (procedure 9) as defined in RFC 1813 section 3.3.9
/// Used to create a new directory
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct MKDIR3args {
    pub dirops: diropargs3,
    pub attributes: sattr3,
}
DeserializeStruct!(MKDIR3args, dirops, attributes);
SerializeStruct!(MKDIR3args, dirops, attributes);

/// Directory entry returned by READDIR operation as defined in RFC 1813 section 3.3.16
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct entry3 {
    pub fileid: fileid3,
    pub name: filename3,
    pub cookie: cookie3,
}
DeserializeStruct!(entry3, fileid, name, cookie);
SerializeStruct!(entry3, fileid, name, cookie);

/// Arguments for the READDIR procedure (procedure 16) as defined in RFC 1813 section 3.3.16
/// Used to read entries from a directory
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct READDIR3args {
    pub dir: nfs_fh3,
    pub cookie: cookie3,
    pub cookieverf: cookieverf3,
    pub dircount: count3,
}
DeserializeStruct!(READDIR3args, dir, cookie, cookieverf, dircount);
SerializeStruct!(READDIR3args, dir, cookie, cookieverf, dircount);

/// Directory entry with additional attributes for READDIRPLUS operation
/// as defined in RFC 1813 section 3.3.17
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct entryplus3 {
    pub fileid: fileid3,
    pub name: filename3,
    pub cookie: cookie3,
    pub name_attributes: post_op_attr,
    pub name_handle: post_op_fh3,
}
DeserializeStruct!(entryplus3, fileid, name, cookie, name_attributes, name_handle);
SerializeStruct!(entryplus3, fileid, name, cookie, name_attributes, name_handle);

/// Arguments for the READDIRPLUS procedure (procedure 17) as defined in RFC 1813 section 3.3.17
/// Used to read directory entries with extended information
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct READDIRPLUS3args {
    pub dir: nfs_fh3,
    pub cookie: cookie3,
    pub cookieverf: cookieverf3,
    pub dircount: count3,
    pub maxcount: count3,
}
DeserializeStruct!(READDIRPLUS3args, dir, cookie, cookieverf, dircount, maxcount);
SerializeStruct!(READDIRPLUS3args, dir, cookie, cookieverf, dircount, maxcount);
