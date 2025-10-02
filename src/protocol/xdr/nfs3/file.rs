//! Module contains XDR data structures related to file operations for NFS version 3 protocol
//! as defined in RFC 1813.
//!
//! This module includes data structures for the following operations:
//! - READ: Read data from a file (procedure 6)
//! - WRITE: Write data to a file (procedure 7)
//! - COMMIT: Commit asynchronously written data to stable storage (procedure 21)
//! - LINK: Create a hard link (procedure 15)
//!
//! The structures implement the XDR serialization/deserialization interfaces for
//! the request arguments and response data of these operations.

// Allow unused code warnings since we implement the complete RFC 1813 specification,
// including procedures that may not be used by all clients
#![allow(dead_code)]
// Preserve original RFC naming conventions (e.g. READ3args, COMMIT3resok)
// for consistency with the NFS version 3 protocol specification

use std::io::{Read, Write};

use num_derive::{FromPrimitive, ToPrimitive};

use super::{
    Count3, Deserialize, DeserializeEnum, DeserializeStruct, DiropArgs3, NFSFh3, Offset3,
    PostOpAttr, Serialize, SerializeEnum, SerializeStruct, WCCData, WriteVerf3,
};

/// Arguments for the READ procedure (procedure 6) as defined in RFC 1813 section 3.3.6
/// Used to read data from a regular file
#[derive(Debug, Default)]
pub struct READ3args {
    /// File handle for the file to be read
    pub file: NFSFh3,
    /// Position within the file to begin reading
    pub offset: Offset3,
    /// Number of bytes of data to read
    pub count: Count3,
}
DeserializeStruct!(READ3args, file, offset, count);
SerializeStruct!(READ3args, file, offset, count);

/// Successful response for the READ procedure as defined in RFC 1813 section 3.3.6
#[derive(Debug, Default)]
pub struct READ3resok {
    /// File attributes after the operation
    pub file_attributes: PostOpAttr,
    /// Number of bytes actually read
    pub count: Count3,
    /// True if the end of file was reached
    pub eof: bool,
    /// The data read from the file
    pub data: Vec<u8>,
}
DeserializeStruct!(READ3resok, file_attributes, count, eof, data);
SerializeStruct!(READ3resok, file_attributes, count, eof, data);

/// Arguments for the COMMIT procedure (procedure 21) as defined in RFC 1813 section 3.3.21
/// Used to commit pending writes to stable storage
#[derive(Debug, Default)]
pub struct COMMIT3args {
    /// File handle for the file to commit
    pub file: NFSFh3,
    /// Position within the file to start committing
    pub offset: Offset3,
    /// Number of bytes to commit
    pub count: Count3,
}
DeserializeStruct!(COMMIT3args, file, offset, count);
SerializeStruct!(COMMIT3args, file, offset, count);

/// Successful response for the COMMIT procedure as defined in RFC 1813 section 3.3.21
#[derive(Debug, Default)]
pub struct COMMIT3resok {
    /// File attributes before and after the operation
    pub file_wcc: WCCData,
    /// Write verifier to detect server restarts
    pub verf: WriteVerf3,
}
DeserializeStruct!(COMMIT3resok, file_wcc, verf);
SerializeStruct!(COMMIT3resok, file_wcc, verf);

/// Arguments for the LINK procedure (procedure 15) as defined in RFC 1813 section 3.3.15
/// Used to create a hard link to a file
#[derive(Debug, Default)]
pub struct LINK3args {
    /// File handle for the target file
    pub file: NFSFh3,
    /// Directory and name for the new link
    pub link: DiropArgs3,
}
DeserializeStruct!(LINK3args, file, link);
SerializeStruct!(LINK3args, file, link);

/// Enumeration specifying how data should be written to storage
/// as defined in RFC 1813 section 3.3.7
#[derive(Copy, Clone, Debug, Default, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum StableHow {
    /// Data may be buffered before writing to stable storage
    /// The server may return before the data is committed to stable storage
    #[default]
    Unstable = 0,
    /// Data must be committed to stable storage before returning
    /// Only the data for this request is guaranteed to be committed
    DataSync = 1,
    /// All file system data must be committed to stable storage before returning
    /// This includes the data and all metadata for this request
    FileSync = 2,
}
impl SerializeEnum for StableHow {}
impl DeserializeEnum for StableHow {}

/// Arguments for the WRITE procedure (procedure 7) as defined in RFC 1813 section 3.3.7
/// Used to write data to a regular file
#[derive(Debug, Default)]
pub struct WRITE3args {
    /// File handle for the file to write
    pub file: NFSFh3,
    /// Position within the file to begin writing
    pub offset: Offset3,
    /// Number of bytes of data to write
    pub count: Count3,
    /// How to commit the data to storage
    pub stable: u32,
    /// The data to be written
    pub data: Vec<u8>,
}
DeserializeStruct!(WRITE3args, file, offset, count, stable, data);
SerializeStruct!(WRITE3args, file, offset, count, stable, data);

/// Successful response for the WRITE procedure as defined in RFC 1813 section 3.3.7
#[derive(Debug, Default)]
pub struct WRITE3resok {
    /// File attributes before and after the operation
    pub file_wcc: WCCData,
    /// Number of bytes actually written
    pub count: Count3,
    /// How the data was committed to stable storage
    pub committed: StableHow,
    /// Write verifier to detect server restarts
    pub verf: WriteVerf3,
}
DeserializeStruct!(WRITE3resok, file_wcc, count, committed, verf);
SerializeStruct!(WRITE3resok, file_wcc, count, committed, verf);
