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

use std::io::{Read, Write};

use crate::xdr::nfs3::nfs_fh3;

use super::{
    nfstime3, post_op_attr, size3, Deserialize, DeserializeStruct, Serialize, SerializeStruct,
};

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

/// Arguments for the FSINFO procedure (procedure 19) as defined in RFC 1813 section 3.3.19
/// Used to get static file system information
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct FSINFO3args {
    pub fsroot: nfs_fh3,
}
DeserializeStruct!(FSINFO3args, fsroot);
SerializeStruct!(FSINFO3args, fsroot);

/// File system information structure returned by FSINFO procedure
/// as defined in RFC 1813 section 3.3.19
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct fsinfo3 {
    /// File system attributes
    pub obj_attributes: post_op_attr,
    pub rtmax: u32,
    pub rtpref: u32,
    pub rtmult: u32,
    pub wtmax: u32,
    pub wtpref: u32,
    pub wtmult: u32,
    pub dtpref: u32,
    pub maxfilesize: size3,
    pub time_delta: nfstime3,
    pub properties: u32,
}
DeserializeStruct!(
    fsinfo3,
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
    fsinfo3,
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

/// Arguments for the FSSTAT procedure (procedure 18) as defined in RFC 1813 section 3.3.18
/// Used to get file system statistics
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct FSSTAT3args {
    pub fsroot: nfs_fh3,
}
DeserializeStruct!(FSSTAT3args, fsroot);
SerializeStruct!(FSSTAT3args, fsroot);

/// File system statistics returned by FSSTAT procedure
/// as defined in RFC 1813 section 3.3.18
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct FSSTAT3resok {
    pub obj_attributes: post_op_attr,
    pub tbytes: size3,
    pub fbytes: size3,
    pub abytes: size3,
    pub tfiles: size3,
    pub ffiles: size3,
    pub afiles: size3,
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

/// Arguments for the PATHCONF procedure (procedure 20) as defined in RFC 1813 section 3.3.20
/// Used to get path configuration information
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct PATHCONF3args {
    pub object: nfs_fh3,
}
DeserializeStruct!(PATHCONF3args, object);
SerializeStruct!(PATHCONF3args, object);

/// Path configuration information returned by PATHCONF procedure
/// as defined in RFC 1813 section 3.3.20
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct PATHCONF3resok {
    pub obj_attributes: post_op_attr,
    pub linkmax: u32,
    pub name_max: u32,
    pub no_trunc: bool,
    pub chown_restricted: bool,
    pub case_insensitive: bool,
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
