//! Context shared while executing a single NFSv4 COMPOUND procedure.

use crate::rpc::auth::Credential;
use crate::vfs::nfsv4::compound;
use crate::Buffer;

/// Kind of a server-issued filehandle, as described by the `fh_expire_type`
/// attribute (`fh_expire_type4`), RFC 7530 §5.8.1.3.
///
/// The attribute is a bitmask of the `FH4_*` flags in which `FH4_PERSISTENT`
/// is the value with none of the volatile bits set. This type collapses the
/// wire bitmask into the meaningful stability classes so an implementation can
/// decide whether a handle may expire under the object's use.
pub enum FileHandle {
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
const FH4_NOEXPIRE_WITH_OPEN: u32 = 0x00000001;
const FH4_VOLATILE_ANY: u32 = 0x00000002;
const FH4_VOL_MIGRATION: u32 = 0x00000004;
const FH4_VOL_RENAME: u32 = 0x00000008;

impl FileHandle {
    /// Builds a [`FileHandle`] from the raw `fh_expire_type` bitmask.
    ///
    /// The bitmask is interpreted per RFC 7530 §5.8.1.3: a mask with no
    /// volatile bits set denotes a persistent handle, otherwise the most
    /// specific matching volatile classification wins. Unknown bits are
    /// ignored.
    pub fn from_bits(bits: u32) -> FileHandle {
        let volatile_any = bits & FH4_VOLATILE_ANY != 0;
        let noexpire_open = bits & FH4_NOEXPIRE_WITH_OPEN != 0;
        let migration = bits & FH4_VOL_MIGRATION != 0;
        let rename = bits & FH4_VOL_RENAME != 0;

        if !volatile_any && !noexpire_open && !migration && !rename {
            FileHandle::Persistent
        } else if volatile_any && noexpire_open {
            FileHandle::VolatileNotOpen
        } else if volatile_any {
            FileHandle::VolatileAny
        } else if migration && rename {
            FileHandle::VolatileMigrationOrRename
        } else if migration {
            FileHandle::VolatileMigration
        } else if rename {
            FileHandle::VolatileRename
        } else {
            // Only `FH4_NOEXPIRE_WITH_OPEN` set: it may only be set together
            // with `FH4_VOLATILE_ANY`, so fall back to the generic volatile.
            //TODO: change to actual error, when whole pipeline wold be set
            FileHandle::VolatileAny
        }
    }

    /// Returns `true` if the handle may expire at some point, i.e. it is not
    /// [`FileHandle::Persistent`].
    pub fn is_volatile(self) -> bool {
        !matches!(self, FileHandle::Persistent)
    }

    /// Returns the `fh_expire_type` bitmask this [`FileHandle`] represents.
    pub fn bits(self) -> u32 {
        match self {
            FileHandle::Persistent => 0x0,
            FileHandle::VolatileAny => FH4_VOLATILE_ANY,
            FileHandle::VolatileNotOpen => FH4_VOLATILE_ANY | FH4_NOEXPIRE_WITH_OPEN,
            FileHandle::VolatileMigration => FH4_VOL_MIGRATION,
            FileHandle::VolatileRename => FH4_VOL_RENAME,
            FileHandle::VolatileMigrationOrRename => FH4_VOL_MIGRATION | FH4_VOL_RENAME,
        }
    }
}

/// Execution context of a single NFSv4 COMPOUND procedure.
///
/// Carries the request header fields from [`compound::Args`] together with the
/// file handle state maintained across the operations of the compound: the
/// current and saved file handles. It is constructed on entry to the COMPOUND
/// procedure and threaded through the execution of the operations.
#[allow(dead_code)]
pub struct CompoundContext<B: Buffer> {
    /// Client-supplied tag echoed back in the compound result.
    tag: compound::Tag,
    /// Authenticated identity of the caller.
    credentials: Credential,
    /// Current file handle, `None` before one is established.
    current: Option<FileHandle>,
    /// Saved file handle, populated and cleared by SAVEFH/RESTOREFH.
    saved: Option<FileHandle>,
    /// Current operation in `ops` vector
    position: usize,
    /// Operations of the compound in execution order.
    ops: Vec<compound::OperationArgs<B>>,
}

impl<B: Buffer> CompoundContext<B> {
    /// Creates a context for a COMPOUND procedure.
    ///
    /// `tag`, `credentials`, and `ops` are taken from the decoded request.
    pub fn new(
        tag: compound::Tag,
        credentials: Credential,
        ops: Vec<compound::OperationArgs<B>>,
    ) -> Self {
        CompoundContext { tag, credentials, current: None, saved: None, position: 0, ops }
    }

    /// Returns the authenticated caller identity.
    pub fn credentials(&self) -> &Credential {
        &self.credentials
    }

    /// Returns the operation at `index`, if present.
    pub fn op(&self, index: usize) -> Option<&compound::OperationArgs<B>> {
        self.ops.get(index)
    }

    /// Sets the current file handle.
    pub fn set_current(&mut self, fh: FileHandle) {
        self.current = Some(fh);
    }

    /// Sets the saved file handle.
    pub fn set_saved(&mut self, fh: FileHandle) {
        self.saved = Some(fh);
    }
}
