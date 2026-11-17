use crate::consts::nfsv4::{MAX_NAME_LEN, NFS4_FHSIZE, NFS4_OTHER_SIZE};
use crate::vfs;

/// Kind of a server-issued file handle, as described by the `fh_expire_type`
/// attribute (`fh_expire_type4`), RFC 7530 §5.8.1.3.
///
/// On the wire the attribute is a bitmask of the `FH4_*` flags in which
/// `FH4_PERSISTENT` is the value with none of the volatile bits set. This type
/// collapses that bitmask into the meaningful stability classes so an
/// implementation can decide whether a handle may expire under the object's
/// use. It is stored in the first four bits of every [`Handle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    /// FH4_PERSISTENT - valid until the object is removed from the file
    /// system; the server never returns NFS4ERR_FHEXPIRED for such a handle.
    Persistent,
    /// FH4_VOLATILE_ANY - may expire at any time.
    VolatileAny,
    /// FH4_VOLATILE_ANY | FH4_NOEXPIRE_WITH_OPEN - may expire at any time
    /// except while the object it refers to is open.
    VolatileNotOpen,
    /// FH4_VOL_MIGRATION - expires as the object is migrated to another
    /// server.
    VolatileMigration,
    /// FH4_VOL_RENAME - expires when the object is renamed, by this client or
    /// any other.
    VolatileRename,
    /// FH4_VOL_MIGRATION | FH4_VOL_RENAME - expires on migration or rename.
    VolatileMigrationOrRename,
}

/// The `FH4_*` flag values, RFC 7531 §2.
const FH4_NOEXPIRE_WITH_OPEN: u8 = 0x01;
const FH4_VOLATILE_ANY: u8 = 0x02;
const FH4_VOL_MIGRATION: u8 = 0x04;
const FH4_VOL_RENAME: u8 = 0x08;

impl Type {
    /// Builds a [`Type`] from the raw `fh_expire_type` bitmask.
    ///
    /// The bitmask is interpreted per RFC 7530 §5.8.1.3: a mask with no
    /// volatile bits set denotes a persistent handle, otherwise the most
    /// specific matching volatile classification wins. Unknown bits are
    /// ignored.
    pub fn from_bits(bits: u8) -> Type {
        let volatile_any = bits & FH4_VOLATILE_ANY != 0;
        let noexpire_open = bits & FH4_NOEXPIRE_WITH_OPEN != 0;
        let migration = bits & FH4_VOL_MIGRATION != 0;
        let rename = bits & FH4_VOL_RENAME != 0;

        if !volatile_any && !noexpire_open && !migration && !rename {
            Type::Persistent
        } else if volatile_any && noexpire_open {
            Type::VolatileNotOpen
        } else if volatile_any {
            Type::VolatileAny
        } else if migration && rename {
            Type::VolatileMigrationOrRename
        } else if migration {
            Type::VolatileMigration
        } else if rename {
            Type::VolatileRename
        } else {
            // Only `FH4_NOEXPIRE_WITH_OPEN` set: it may only be set together
            // with `FH4_VOLATILE_ANY`, so fall back to the generic volatile.
            //TODO: change to actual error, when whole pipeline wold be set
            Type::VolatileAny
        }
    }

    /// Returns `true` if the handle may expire at some point, i.e. it is not
    /// [`Type::Persistent`].
    pub fn is_volatile(self) -> bool {
        !matches!(self, Type::Persistent)
    }

    /// Returns the `fh_expire_type` bitmask this [`Type`] represents.
    pub fn bits(self) -> u8 {
        match self {
            Type::Persistent => 0x0,
            Type::VolatileAny => FH4_VOLATILE_ANY,
            Type::VolatileNotOpen => FH4_VOLATILE_ANY | FH4_NOEXPIRE_WITH_OPEN,
            Type::VolatileMigration => FH4_VOL_MIGRATION,
            Type::VolatileRename => FH4_VOL_RENAME,
            Type::VolatileMigrationOrRename => FH4_VOL_MIGRATION | FH4_VOL_RENAME,
        }
    }

    /// Returns the on-wire representation stored in the first four bits of a
    /// [`Handle`].
    #[inline]
    pub(crate) fn to_bits(self) -> u8 {
        self.bits() & TYPE_MASK
    }

    /// Builds a [`Type`] from the first four bits of a [`Handle`].
    #[inline]
    pub(crate) fn from_handle_bits(bits: u8) -> Type {
        Type::from_bits(bits & TYPE_MASK)
    }
}

/// Mask selecting the [`Type`] nibble at the start of a [`Handle`].
const TYPE_MASK: u8 = 0x0F;

/// Size of the backend-specific part of a [`Handle`], in bytes.
///
/// Every [`Handle`] is [`NFS4_FHSIZE`] bytes long; the first four bits carry
/// the [`Type`], which leaves the remaining bits and bytes opaque to the
/// server.
pub const PAYLOAD_SIZE: usize = NFS4_FHSIZE;

/// A file handle (`nfs_fh4`).
///
/// Corresponds to the file handle from RFC 7530. It is an opaque byte array
/// of at most [`NFS4_FHSIZE`] bytes, interpreted only by the server.
///
/// The first four bits of the first byte are the [`Type`] of the handle; the
/// remaining [`PAYLOAD_SIZE`] bytes are opaque to the server and interpreted
/// only by the backend that owns the object.
///
/// TODO: <https://github.com/RMamonts/nfs-mamont/issues/496>
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Handle([u8; NFS4_FHSIZE]);

impl Handle {
    /// Creates a new [`Handle`] of kind `kind` from a backend-specific
    /// `payload`.
    pub fn new(kind: Type, payload: [u8; PAYLOAD_SIZE]) -> Self {
        let mut raw = payload;
        raw[0] = (raw[0] & !TYPE_MASK) | kind.to_bits();
        Handle(raw)
    }

    /// Creates a new [`Handle`] from the raw bytes as received from the NFS
    /// client.
    pub(crate) fn from_bytes(raw: [u8; NFS4_FHSIZE]) -> Self {
        Handle(raw)
    }

    /// Returns the kind of the handle.
    pub fn kind(&self) -> Type {
        Type::from_handle_bits(self.0[0])
    }

    /// Returns the backend-specific part of the handle.
    pub fn payload(&self) -> [u8; PAYLOAD_SIZE] {
        let mut payload = self.0;
        payload[0] &= !TYPE_MASK;
        payload
    }

    /// Returns the raw bytes of the handle.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Consumes the handle and returns its raw bytes.
    #[inline]
    pub(crate) fn into_bytes(self) -> [u8; NFS4_FHSIZE] {
        self.0
    }
}

/// A sequence identifier (`seqid4`).
///
/// Used to order operations on the same state and detect duplicate or
/// out-of-order requests. See RFC 7530 §2.2.15.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct SeqId(u32);

impl SeqId {
    /// Builds a sequence id from its raw value.
    pub fn new(raw: u32) -> Self {
        SeqId(raw)
    }

    /// Returns the raw value of the sequence id.
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// A client identifier (`clientid4`), RFC 7530 §16.5.
///
/// Uniquely identifies a client instance for the purpose of establishing
/// and maintaining locks, delegations and leases.
pub struct ClientId(u64);

impl ClientId {
    /// Builds a client id from its raw value.
    pub fn new(raw: u64) -> Self {
        ClientId(raw)
    }

    /// Returns the raw value of the client id.
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// A bitmask of attribute names (`bitmap4`), RFC 7530 §2.2.3.
///
/// Each bit corresponds to a file attribute; the mask selects which set of
/// attributes is requested or returned.
pub struct Bitmap(Vec<u32>);

impl Bitmap {
    // TODO: find out, if it should be limited by implementation
    // TODO: for example, limited support of attributes by server
    pub fn new(raw: Vec<u32>) -> Self {
        Bitmap(raw)
    }

    pub fn raw(self) -> Vec<u32> {
        self.0
    }
}

/// A list of attributes
pub struct AttrList(Vec<u8>);

impl AttrList {
    // TODO: same as [`Bitmap`]
    pub fn new(raw: Vec<u8>) -> Self {
        AttrList(raw)
    }

    pub fn raw(self) -> Vec<u8> {
        self.0
    }
}

/// A set of file attributes (`fattr4`), RFC 7530 §2.2.8.
///
/// Carries the attribute mask and the attributes encoded as an opaque
/// byte sequence.
pub struct Attr {
    /// Bitmask describing the attributes carried in `attr_vals`.
    pub attrmask: Bitmap,
    /// Attributes encoded as an opaque byte sequence.
    pub attr_vals: AttrList,
}

/// A state identifier (`stateid4`).
///
/// Used to identify a particular state of the file opened by a client, such
/// as an open file description or a byte-range lock. See RFC 7530 §2.2.16.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StateId {
    /// Monotonically increasing sequence id for the state.
    pub seqid: SeqId,
    /// Opaque [`crate::consts::nfsv4::NFS4_OTHER_SIZE`]-byte value identifying
    /// the state.
    pub other: [u8; NFS4_OTHER_SIZE],
}

/// A validated wrapper around a `String` representing a filename.
///
/// [`Name`] ensures that the inner string does not exceed [`MAX_NAME_LEN`].
pub type Name = vfs::nfs_shared::file::Name<MAX_NAME_LEN>;

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
