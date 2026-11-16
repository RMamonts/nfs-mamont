//! Defines the NFSv4 CREATE operation types, RFC 7530 §16.4.

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

/// A change identifier (`changeid4`), RFC 7530 §2.2.9.
pub struct ChangeId(u64);

impl ChangeId {
    /// Builds a change id from its raw value.
    pub fn new(id: u64) -> ChangeId {
        ChangeId(id)
    }

    /// Returns the raw value of the change id.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

/// Change information for a directory (`change_info4`), RFC 7530 §2.2.9.
pub struct ChangeInfo {
    /// Whether `before` and `after` refer to the same directory.
    pub atomic: bool,
    /// Change attribute before the operation.
    pub before: ChangeId,
    /// Change attribute after the operation.
    pub after: ChangeId,
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

/// A set of file attributes (`fattr4`), RFC 7530 §2.2.8.
///
/// Represents the attributes to set on the newly created object. The raw
/// attribute mask and values are kept opaque.
pub struct Attr {
    /// Bitmask describing the attributes carried in `attr_vals`.
    pub attrmask: Vec<u32>,
    /// Attributes encoded as an opaque byte sequence.
    pub attr_vals: Vec<u8>,
}

/// CREATE arguments (`CREATE4args`).
pub struct Args {
    /// Type and creation data of the object to create.
    pub objtype: CreateType,
    /// Name of the object to create.
    pub objname: String,
    /// Attributes to set on the newly created object.
    pub createattrs: Attr,
}

/// Success result (`CREATE4resok`).
pub struct Success {
    /// Change information for the directory in which the object was created.
    pub cinfo: ChangeInfo,
    /// Bitmask of attributes for which the server could not set the value.
    pub attrset: Vec<u32>,
}

/// Fail result (`CREATE4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
