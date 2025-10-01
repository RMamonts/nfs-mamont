//! Module contains XDR data structures related to operations that can be performed
//! on any type of filesystem object (file, directory, symlink, etc.) for NFS version 3 protocol
//! as defined in RFC 1813.
//!
//! This module includes data structures for the following operations:
//! - ACCESS: Check access permissions (procedure 4)
//! - SYMLINK: Create a symbolic link (procedure 10)
//! - GETATTR: Get file attributes (procedure 1)
//! - MKNOD: Create a special device (procedure 11)
//! - SETATTR: Set file attributes (procedure 2)
//! - RENAME: Rename a file or directory (procedure 14)
//! - READLINK: Read symbolic link (procedure 5)
//! - REMOVE: Remove a file from directory (procedure 12)
//!
//!
//! The structures implement the XDR serialization/deserialization interfaces for
//! the request arguments of these operations.

use crate::xdr::nfs3::{diropargs3, ftype3, nfs_fh3, nfstime3, sattr3, specdata3, symlinkdata3};
use crate::xdr::Serialize;
use crate::xdr::{deserialize, Deserialize};
use crate::{DeserializeStruct, SerializeStruct};
use std::io::Read;
use std::io::Write;

/// Arguments for the ACCESS procedure (procedure 4) as defined in RFC 1813 section 3.3.4
/// Used to check access permissions for a filesystem object
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct ACCESS3args {
    pub object: nfs_fh3,
    pub access: u32,
}
SerializeStruct!(ACCESS3args, object, access);
DeserializeStruct!(ACCESS3args, object, access);

/// Arguments for the GETATTR procedure (procedure 1) as defined in RFC 1813 section 3.3.1
/// Used to get attributes of a filesystem object
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct GETATTR3args {
    pub object: nfs_fh3,
}
SerializeStruct!(GETATTR3args, object);
DeserializeStruct!(GETATTR3args, object);

pub type sattrguard3 = Option<nfstime3>;

/// Arguments for the SETATTR procedure (procedure 2) as defined in RFC 1813 section 3.3.2
/// Used to set attributes of a filesystem object
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
pub struct SETATTR3args {
    pub object: nfs_fh3,
    pub new_attribute: sattr3,
    pub guard: sattrguard3,
}
DeserializeStruct!(SETATTR3args, object, new_attribute, guard);
SerializeStruct!(SETATTR3args, object, new_attribute, guard);

/// Arguments for the RENAME procedure (procedure 14) as defined in RFC 1813 section 3.3.14
/// Used to rename a filesystem object
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct RENAME3args {
    pub from: diropargs3,
    pub to: diropargs3,
}
SerializeStruct!(RENAME3args, from, to);
DeserializeStruct!(RENAME3args, from, to);

/// Arguments for the READLINK procedure (procedure 5) as defined in RFC 1813 section 3.3.5
/// Used to read the contents of a symbolic link
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct READLINK3args {
    pub symlink: nfs_fh3,
}
SerializeStruct!(READLINK3args, symlink);
DeserializeStruct!(READLINK3args, symlink);

/// Arguments for the REMOVE procedure (procedure 12) as defined in RFC 1813 section 3.3.12
/// Used to remove a file from a directory
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct REMOVE3args {
    pub object: diropargs3,
}
SerializeStruct!(REMOVE3args, object);
DeserializeStruct!(REMOVE3args, object);

/// Arguments for the SYMLINK procedure (procedure 10)
/// as defined in RFC 1813 section 3.3.10
/// Used to create a symbolic link
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct SYMLINK3args {
    /// Directory where symbolic link should be created and its name
    pub dirops: diropargs3,
    /// Target path and attributes for the symbolic link
    pub symlink: symlinkdata3,
}
DeserializeStruct!(SYMLINK3args, dirops, symlink);
SerializeStruct!(SYMLINK3args, dirops, symlink);

/// Arguments for the MKNOD procedure (procedure 11)
/// as defined in RFC 1813 section 3.3.11
/// Used to create a special device file, FIFO, or socket
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct MKNOD3args {
    /// Directory where the special file should be created and its name
    pub where_dir: diropargs3,
    /// Type and device information for the special file
    pub what: mknoddata3,
}
DeserializeStruct!(MKNOD3args, where_dir, what);
SerializeStruct!(MKNOD3args, where_dir, what);

/// Device data for special files
/// as defined in RFC 1813 section 3.3.11
/// Contains the device type and device numbers
#[allow(non_camel_case_types)]
#[derive(Debug, Default)]
pub struct devicedata3 {
    /// Type of device (character, block, socket, or FIFO)
    pub attr: sattr3,
    /// Major and minor device numbers for character and block devices
    pub device: specdata3,
}
DeserializeStruct!(devicedata3, attr, device);
SerializeStruct!(devicedata3, attr, device);

/// Data structure for creating special files
/// as defined in RFC 1813 section 3.3.11
/// Contains the file type and device information
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum mknoddata3 {
    NF3REG,
    NF3DIR,
    NF3LNK,
    NF3CHR(devicedata3),
    NF3BLK(devicedata3),
    NF3SOCK(sattr3),
    NF3FIFO(sattr3),
}

impl Default for mknoddata3 {
    fn default() -> Self {
        Self::NF3REG
    }
}

impl Serialize for mknoddata3 {
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
        match self {
            mknoddata3::NF3REG => ftype3::NF3REG.serialize(dest),
            mknoddata3::NF3DIR => ftype3::NF3DIR.serialize(dest),
            mknoddata3::NF3LNK => ftype3::NF3LNK.serialize(dest),
            mknoddata3::NF3CHR(arg) => {
                ftype3::NF3CHR.serialize(dest)?;
                arg.serialize(dest)
            }
            mknoddata3::NF3BLK(arg) => {
                ftype3::NF3BLK.serialize(dest)?;
                arg.serialize(dest)
            }
            mknoddata3::NF3SOCK(arg) => {
                ftype3::NF3SOCK.serialize(dest)?;
                arg.serialize(dest)
            }
            mknoddata3::NF3FIFO(arg) => {
                ftype3::NF3FIFO.serialize(dest)?;
                arg.serialize(dest)
            }
        }
    }
}

impl Deserialize for mknoddata3 {
    fn deserialize<R: Read>(src: &mut R) -> std::io::Result<Self> {
        let ftype = deserialize::<ftype3>(src)?;
        match ftype {
            ftype3::NF3REG => Ok(mknoddata3::NF3REG),
            ftype3::NF3DIR => Ok(mknoddata3::NF3DIR),
            ftype3::NF3BLK => {
                let arg = deserialize::<devicedata3>(src)?;
                Ok(mknoddata3::NF3BLK(arg))
            }
            ftype3::NF3CHR => {
                let arg = deserialize::<devicedata3>(src)?;
                Ok(mknoddata3::NF3CHR(arg))
            }
            ftype3::NF3LNK => Ok(mknoddata3::NF3LNK),
            ftype3::NF3SOCK => {
                let arg = deserialize::<sattr3>(src)?;
                Ok(mknoddata3::NF3SOCK(arg))
            }
            ftype3::NF3FIFO => {
                let arg = deserialize::<sattr3>(src)?;
                Ok(mknoddata3::NF3FIFO(arg))
            }
        }
    }
}
