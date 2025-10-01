//! Module contains XDR data structures related to file operations for NFS version 3 protocol
//! as defined in RFC 1813.
//!
//! This module includes data structures for the following operations:
//! - READ: Read data from a file (procedure 6)
//! - WRITE: Write data to a file (procedure 7)
//! - COMMIT: Commit asynchronously written data to stable storage (procedure 21)
//! - LINK: Create a hard link (procedure 15)
//! - LOOKUP: Look up a file name in a directory (procedure 3)
//! - CREATE: Create a regular file (procedure 8)
//!
//! The structures implement the XDR serialization/deserialization interfaces for
//! the request arguments and response data of these operations.

use std::io::{Read, Write};

use num_derive::{FromPrimitive, ToPrimitive};

use crate::xdr::deserialize;
use crate::xdr::nfs3::{createverf3, sattr3};
use crate::{DeserializeTypeEnum, SerializeTypeEnum};

use super::{
    count3, diropargs3, nfs_fh3, offset3, post_op_attr, wcc_data, writeverf3, Deserialize,
    DeserializeEnum, DeserializeStruct, Serialize, SerializeEnum, SerializeStruct,
};

/// Arguments for the READ procedure (procedure 6) as defined in RFC 1813 section 3.3.6
/// Used to read data from a regular file
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct READ3args {
    pub file: nfs_fh3,
    pub offset: offset3,
    pub count: count3,
}
DeserializeStruct!(READ3args, file, offset, count);
SerializeStruct!(READ3args, file, offset, count);

/// Successful response for the READ procedure as defined in RFC 1813 section 3.3.6
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct READ3resok {
    pub file_attributes: post_op_attr,
    pub count: count3,
    pub eof: bool,
    pub data: Vec<u8>,
}
DeserializeStruct!(READ3resok, file_attributes, count, eof, data);
SerializeStruct!(READ3resok, file_attributes, count, eof, data);

/// Arguments for the COMMIT procedure (procedure 21) as defined in RFC 1813 section 3.3.21
/// Used to commit pending writes to stable storage
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct COMMIT3args {
    pub file: nfs_fh3,
    pub offset: offset3,
    pub count: count3,
}
DeserializeStruct!(COMMIT3args, file, offset, count);
SerializeStruct!(COMMIT3args, file, offset, count);

/// Successful response for the COMMIT procedure as defined in RFC 1813 section 3.3.21
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct COMMIT3resok {
    pub file_wcc: wcc_data,
    pub verf: writeverf3,
}
DeserializeStruct!(COMMIT3resok, file_wcc, verf);
SerializeStruct!(COMMIT3resok, file_wcc, verf);

/// Arguments for the LINK procedure (procedure 15) as defined in RFC 1813 section 3.3.15
/// Used to create a hard link to a file
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct LINK3args {
    pub file: nfs_fh3,
    pub link: diropargs3,
}
DeserializeStruct!(LINK3args, file, link);
SerializeStruct!(LINK3args, file, link);

/// Enumeration specifying how data should be written to storage
/// as defined in RFC 1813 section 3.3.7
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Default, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum stable_how {
    #[default]
    UNSTABLE = 0,
    DATA_SYNC = 1,
    FILE_SYNC = 2,
}
impl SerializeEnum for stable_how {}
impl DeserializeEnum for stable_how {}

/// Arguments for the WRITE procedure (procedure 7) as defined in RFC 1813 section 3.3.7
/// Used to write data to a regular file
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct WRITE3args {
    pub file: nfs_fh3,
    pub offset: offset3,
    pub count: count3,
    pub stable: stable_how,
    pub data: Vec<u8>,
}
DeserializeStruct!(WRITE3args, file, offset, count, stable, data);
SerializeStruct!(WRITE3args, file, offset, count, stable, data);

/// Successful response for the WRITE procedure as defined in RFC 1813 section 3.3.7
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct WRITE3resok {
    pub file_wcc: wcc_data,
    pub count: count3,
    pub committed: stable_how,
    pub verf: writeverf3,
}
DeserializeStruct!(WRITE3resok, file_wcc, count, committed, verf);
SerializeStruct!(WRITE3resok, file_wcc, count, committed, verf);

/// Arguments for the LOOKUP procedure (procedure 3) as defined in RFC 1813 section 3.3.3
/// Used to look up a file name in a directory
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct LOOKUP3args {
    pub object: diropargs3,
}
SerializeStruct!(LOOKUP3args, object);
DeserializeStruct!(LOOKUP3args, object);

/// File creation modes for `CREATE` operations
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
pub enum createhow3 {
    UNCHECKED(sattr3),
    GUARDED(sattr3),
    EXCLUSIVE(createverf3),
}

impl Default for createhow3 {
    fn default() -> Self {
        Self::UNCHECKED(sattr3::default())
    }
}

DeserializeTypeEnum!(createhow3; UNCHECKED=0, GUARDED=1, EXCLUSIVE=2);
SerializeTypeEnum!(createhow3; UNCHECKED=0, GUARDED=1, EXCLUSIVE=2);

/// Arguments for the CREATE procedure (procedure 8) as defined in RFC 1813 section 3.3.8
/// Used to create a regular file
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct CREATE3args {
    pub dirops: diropargs3,
    pub how: createhow3,
}
DeserializeStruct!(CREATE3args, dirops, how);
SerializeStruct!(CREATE3args, dirops, how);
