//! Defines the NFSv4 CREATE operation types, RFC 7530 §16.4.

use crate::vfs::nfsv4::file::{Attr, Bitmap, ChangeInfo, Name};

/// File type (`nfs_ftype4`), RFC 7530 §2.2.
pub enum FType {
    /// NF4REG - regular file.
    Regular,
    /// NF4DIR - directory.
    Dir,
    /// NF4BLK - block device.
    Block,
    /// NF4CHR - character device.
    Char,
    /// NF4LNK - symbolic link.
    Link,
    /// NF4SOCK - socket.
    Socket,
    /// NF4FIFO - fifo.
    Fifo,
    /// NF4ATTRDIR - attribute directory.
    AttrDir,
    /// NF4NAMEDATTR - named attribute.
    NamedAttr,
}

/// Device number pairs used by block and character special files
/// (`specdata4`), RFC 7530 §2.2.4.
pub struct SpecData {
    /// Major device number.
    pub specdata1: u32,
    /// Minor device number.
    pub specdata2: u32,
}

/// Type of the file system object to create (`createtype4`), RFC 7530 §16.4.2.
pub enum CreateType {
    /// NF4DIR - create a directory.
    Dir,
    /// NF4BLK - create a block device with the given device data.
    Block(SpecData),
    /// NF4CHR - create a character device with the given device data.
    Char(SpecData),
    /// NF4LNK - create a symbolic link with the given link data.
    Link(Vec<u8>),
    /// NF4SOCK - create a socket.
    Socket,
    /// NF4FIFO - create a fifo.
    Fifo,
}

/// CREATE arguments (`CREATE4args`).
pub struct Args {
    /// Type and creation data of the object to create.
    pub objtype: CreateType,
    /// Name of the object to create.
    pub objname: Name,
    /// Attributes to set on the newly created object.
    pub createattrs: Attr,
}

/// Success result (`CREATE4resok`).
pub struct Success {
    /// Change information for the directory in which the object was created.
    pub cinfo: ChangeInfo,
    /// Bitmask of attributes for which the server could not set the value.
    pub attrset: Bitmap,
}

/// Fail result (`CREATE4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
