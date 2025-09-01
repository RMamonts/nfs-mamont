//! This module implements file system operation types defined in RFC 1813 (NFS Version 3 Protocol)
//! for operations related to file system information and statistics.
//!
//! It includes data structures for the following operations:
//! - FSSTAT: Get file system statistics (procedure 18)
//! - FSINFO: Get file system information (procedure 19)
//! - PATHCONF: Get path configuration information (procedure 20)
//!
//! These structures implement the XDR serialization/deserialization interfaces for
//! file system information requests and responses.

// Allow unused code since we implement the complete RFC specification
#![allow(dead_code)]
// Preserve original RFC naming conventions for consistency with the specification
#![allow(non_camel_case_types)]

use std::io::{Read, Write};

use super::{nfs_fh3, nfstime3, post_op_attr, size3, DeserializeStruct, SerializeStruct};

// Section 3.3.19. Procedure 19: FSINFO - Get static file system Information
// The following constants are used in fsinfo to construct the bitmask 'properties',
// which represents the file system properties.

/// If this bit is 1 (TRUE), the file system supports hard links.
/// As defined in RFC 1813 section 3.3.19.
pub const FSF_LINK: u32 = 0x0001;

/// If this bit is 1 (TRUE), the file system supports symbolic links.
/// As defined in RFC 1813 section 3.3.19.
pub const FSF_SYMLINK: u32 = 0x0002;

/// If this bit is 1 (TRUE), the information returned by
/// PATHCONF is identical for every file and directory
/// in the file system. If it is 0 (FALSE), the client
/// should retrieve PATHCONF information for each file
/// and directory as required.
/// As defined in RFC 1813 section 3.3.19.
pub const FSF_HOMOGENEOUS: u32 = 0x0008;

/// If this bit is 1 (TRUE), the server will set the
/// times for a file via SETATTR if requested (to the
/// accuracy indicated by `time_delta`). If it is 0
/// (FALSE), the server cannot set times as requested.
/// As defined in RFC 1813 section 3.3.19.
pub const FSF_CANSETTIME: u32 = 0x0010;

/// Arguments for the FSSTAT procedure (procedure 18) as defined in RFC 1813 section 3.3.18
#[derive(Debug, Default)]
pub struct FSSTAT3args {
    /// File handle identifying an object in the file system
    pub fsroot: nfs_fh3,
}
DeserializeStruct!(FSSTAT3args, fsroot);
SerializeStruct!(FSSTAT3args, fsroot);

/// File system statistics returned by FSSTAT procedure as defined in RFC 1813 section 3.3.18
#[derive(Debug, Default)]
pub struct FSSTAT3resok {
    /// File system attributes
    pub obj_attributes: post_op_attr,
    /// Total size of file system in bytes
    pub tbytes: size3,
    /// Free space in bytes
    pub fbytes: size3,
    /// Free space available to user in bytes (considering quotas)
    pub abytes: size3,
    /// Total number of file slots
    pub tfiles: size3,
    /// Number of free file slots
    pub ffiles: size3,
    /// Number of free file slots available to user (considering quotas)
    pub afiles: size3,
    /// Time for which this information is valid (seconds)
    /// Zero means the information is always valid
    pub invarsec: u32,
}
DeserializeStruct!(
    FSSTAT3resok,
    obj_attributes,
    tbytes,
    fbytes,
    abytes,
    tfiles,
    ffiles,
    afiles,
    invarsec
);
SerializeStruct!(
    FSSTAT3resok,
    obj_attributes,
    tbytes,
    fbytes,
    abytes,
    tfiles,
    ffiles,
    afiles,
    invarsec
);

/// Failed response for the FSSTAT procedure as defined in RFC 1813 section 3.3.18
#[derive(Debug, Default)]
pub struct FSSTAT3resfail {
    /// Attributes of the file system object specified in fsroot
    pub obj_attributes: post_op_attr,
}
DeserializeStruct!(FSSTAT3resfail, obj_attributes);
SerializeStruct!(FSSTAT3resfail, obj_attributes);

/// Arguments for the FSINFO procedure (procedure 19) as defined in RFC 1813 section 3.3.19
#[derive(Debug, Default)]
pub struct FSINFO3args {
    /// File handle identifying a file object
    pub fsroot: nfs_fh3,
}
DeserializeStruct!(FSINFO3args, fsroot);
SerializeStruct!(FSINFO3args, fsroot);

/// File system information structure returned by FSINFO procedure as defined in RFC 1813 section 3.3.19
#[derive(Debug, Default)]
pub struct FSINFO3resok {
    /// File system attributes
    pub obj_attributes: post_op_attr,
    /// Maximum read request supported by server (bytes)
    pub rtmax: u32,
    /// Preferred read request size (bytes)
    pub rtpref: u32,
    /// Suggested read request multiple (bytes)
    /// Requests should be a multiple of this value
    pub rtmult: u32,
    /// Maximum write request supported by server (bytes)
    pub wtmax: u32,
    /// Preferred write request size (bytes)
    pub wtpref: u32,
    /// Suggested write request multiple (bytes)
    /// Requests should be a multiple of this value
    pub wtmult: u32,
    /// Preferred directory read request size (bytes)
    pub dtpref: u32,
    /// Maximum file size supported (bytes)
    pub maxfilesize: size3,
    /// Server time granularity (resolution of time values)
    pub time_delta: nfstime3,
    /// Bit mask of file system properties (FSF_* constants)
    pub properties: u32,
}
DeserializeStruct!(
    FSINFO3resok,
    obj_attributes,
    rtmax,
    rtpref,
    rtmult,
    wtmax,
    wtpref,
    wtmult,
    dtpref,
    maxfilesize,
    time_delta,
    properties
);
SerializeStruct!(
    FSINFO3resok,
    obj_attributes,
    rtmax,
    rtpref,
    rtmult,
    wtmax,
    wtpref,
    wtmult,
    dtpref,
    maxfilesize,
    time_delta,
    properties
);

/// Failed response for the FSINFO procedure as defined in RFC 1813 section 3.3.19
#[derive(Debug, Default)]
pub struct FSINFO3resfail {
    /// Attributes of the file system object specified in fsroot
    pub obj_attributes: post_op_attr,
}
DeserializeStruct!(FSINFO3resfail, obj_attributes);
SerializeStruct!(FSINFO3resfail, obj_attributes);

/// Arguments for the PATHCONF procedure (procedure 20) as defined in RFC 1813 section 3.3.20
#[derive(Debug, Default)]
pub struct PATHCONF3args {
    /// File handle identifying a file object
    pub object: nfs_fh3,
}
DeserializeStruct!(PATHCONF3args, object);
SerializeStruct!(PATHCONF3args, object);

/// Path configuration information returned by PATHCONF procedure as defined in RFC 1813 section 3.3.20
#[derive(Debug, Default)]
pub struct PATHCONF3resok {
    /// The attributes of the object specified by arguments
    pub obj_attributes: post_op_attr,
    /// Maximum number of hard links to a file
    pub linkmax: u32,
    /// Maximum length of a file name
    pub name_max: u32,
    /// If true, long names are not truncated but return error
    pub no_trunc: bool,
    /// If true, changing file ownership is restricted to privileged users
    pub chown_restricted: bool,
    /// If true, file names are case insensitive (FOO equals foo)
    pub case_insensitive: bool,
    /// If true, file name case is preserved
    pub case_preserving: bool,
}
DeserializeStruct!(
    PATHCONF3resok,
    obj_attributes,
    linkmax,
    name_max,
    no_trunc,
    chown_restricted,
    case_insensitive,
    case_preserving
);
SerializeStruct!(
    PATHCONF3resok,
    obj_attributes,
    linkmax,
    name_max,
    no_trunc,
    chown_restricted,
    case_insensitive,
    case_preserving
);

#[derive(Debug, Default)]
pub struct PATHCONF3resfail {
    /// The attributes of the object specified by arguments
    pub obj_attributes: post_op_attr,
}
DeserializeStruct!(PATHCONF3resfail, obj_attributes);
SerializeStruct!(PATHCONF3resfail, obj_attributes);
