//! Context shared while executing a single NFSv4 COMPOUND procedure.

use crate::rpc::auth::Credential;
use crate::vfs::nfsv4::compound;
use crate::vfs::nfsv4::file::Handle;
use crate::Buffer;

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
    current: Option<Handle>,
    /// Saved file handle, populated and cleared by SAVEFH/RESTOREFH.
    saved: Option<Handle>,
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
    pub fn set_current(&mut self, fh: Handle) {
        self.current = Some(fh);
    }

    /// Sets the saved file handle.
    pub fn set_saved(&mut self, fh: Handle) {
        self.saved = Some(fh);
    }
}
