//! Context shared while executing a single NFSv4 COMPOUND procedure.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::rpc::auth::Credential;
use crate::vfs::nfsv4::compound;
use crate::vfs::nfsv4::file::{ClientId, Handle};
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
    /// Operations of the compound in execution order.
    ops: VecDeque<compound::OperationArgs<B>>,
}

impl<B: Buffer> CompoundContext<B> {
    /// Creates a context for a COMPOUND procedure.
    ///
    /// `tag`, `credentials`, and `ops` are taken from the decoded request.
    pub fn new(
        tag: compound::Tag,
        credentials: Credential,
        ops: VecDeque<compound::OperationArgs<B>>,
    ) -> Self {
        CompoundContext { tag, credentials, current: None, saved: None, ops }
    }

    /// Returns the authenticated caller identity.
    pub fn credentials(&self) -> &Credential {
        &self.credentials
    }

    /// Sets the current file handle.
    pub fn set_current(&mut self, fh: Handle) {
        self.current = Some(fh);
    }

    /// Sets the saved file handle.
    pub fn set_saved(&mut self, fh: Handle) {
        self.saved = Some(fh);
    }

    /// Returns next operation to process or None, if none left
    pub fn next_op(&mut self) -> Option<compound::OperationArgs<B>> {
        self.ops.pop_front()
    }
}

/// A resource held by an NFSv4 client, RFC 7530 §8.
///
/// Carries only the discriminant for now: each variant will grow the state
/// needed to release the resource once the corresponding server state is
/// modelled, e.g. the `stateid` for [`Resource::Open`] and
/// [`Resource::Lock`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resource {
    /// An open file description.
    Open,
    /// A byte-range lock.
    Lock,
    /// A delegation.
    Delegate,
}

/// Server-wide NFSv4 client state.
///
/// Holds what the server must know to run NFSv4 correctly across separate
/// COMPOUND procedures: which client identifiers are still known, and the set
/// of [`Resource`]s each of them owns. When a client's lease expires, or the
/// client is reinitialized with a new `clientid4` (RFC 7530 §16.4 per
/// `NFS4ERR_CLID_INUSE`, §16.5), every resource the dead identifier owned must
/// be released; the client table makes that enumeration possible.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ClientContext {
    /// Client identifiers known to the server, used to tell which are dead.
    clients: HashSet<ClientId>,
    /// Resources owned by each client, keyed by the client identifier.
    resources_by_client: HashMap<ClientId, HashSet<Resource>>,
}

impl ClientContext {
    /// Creates an empty client context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a known client identifier.
    pub fn add_client(&mut self, client: ClientId) {
        self.clients.insert(client);
        self.resources_by_client.entry(client).or_default();
    }

    /// Removes a client identifier and releases every resource it owned.
    pub fn remove_client(&mut self, client: &ClientId) {
        self.clients.remove(client);
        self.resources_by_client.remove(client);
    }

    /// Returns `true` if `client` is a known, not yet expired identifier.
    pub fn is_alive(&self, client: &ClientId) -> bool {
        self.clients.contains(client)
    }

    /// Returns the identifiers of all known clients.
    pub fn clients(&self) -> impl Iterator<Item = &ClientId> {
        self.clients.iter()
    }

    /// Returns the resources owned by `client`, or `None` if the client is not
    /// known.
    pub fn resources(&self, client: &ClientId) -> Option<&HashSet<Resource>> {
        self.resources_by_client.get(client)
    }

    /// Returns the number of resources owned by `client`, or `None` if the
    /// client is not known.
    pub fn resource_count(&self, client: &ClientId) -> Option<usize> {
        self.resources_by_client.get(client).map(HashSet::len)
    }

    /// Records that `client` owns `resource`.
    ///
    /// Ensures the client is known, so the resource can never be recorded
    /// against a dead identifier.
    pub fn add_resource(&mut self, client: ClientId, resource: Resource) {
        self.clients.insert(client);
        self.resources_by_client.entry(client).or_default().insert(resource);
    }

    /// Releases one `resource` owned by `client`.
    ///
    /// A client may keep an empty resource set until [`Self::remove_client`]
    /// removes the identifier itself.
    pub fn remove_resource(&mut self, client: &ClientId, resource: &Resource) {
        if let Some(resources) = self.resources_by_client.get_mut(client) {
            resources.remove(resource);
        }
    }
}
