//! RPC context implementation for maintaining server and client state.
//!
//! The Context module provides the state management infrastructure needed for
//! handling RPC requests. It encapsulates all information required for:
//!
//! - Client identification and authentication
//! - Access to file system resources
//! - Tracking of client sessions and requests
//! - Mount status monitoring
//!
//! This module serves as a bridge between the RPC layer and the underlying
//! file system, providing each protocol handler with the information it needs
//! to process requests correctly in accordance with client permissions and
//! server configuration.

use std::fmt;
use std::sync::{Arc, RwLock};

use tokio::sync::mpsc;

use crate::protocol::nfs::portmap::PortmapTable;
use crate::protocol::xdr;
use crate::vfs;

/// Represents the execution context for RPC operations
///
/// The Context structure encapsulates all the state information needed to process
/// an RPC request, including client identification, authentication credentials,
/// server configuration, and access to the virtual file system.
///
/// This context is passed to all protocol handlers (NFS, MOUNT, PORTMAP), providing
/// them with the information necessary to authenticate, authorize, and execute
/// requested operations. It serves as a bridge between the RPC layer and the
/// underlying file system implementation.
///
/// Each RPC connection maintains its own Context instance, ensuring proper isolation
/// between different client sessions and enabling accurate tracking of client state.
#[derive(Clone)]
pub struct Context {
    /// Port number on which the server is listening
    pub local_port: u16,

    /// Client's network address (IP:port) used for logging and request tracking
    pub client_addr: String,

    /// UNIX-style authentication credentials from the client
    /// Contains user ID, group IDs, and other identity information
    pub auth: xdr::rpc::auth_unix,

    /// Virtual File System implementation that handles actual file operations
    /// Abstracts the underlying storage system for NFS operations
    pub vfs: Arc<dyn vfs::NFSFileSystem + Send + Sync>,

    /// Channel for sending mount/unmount notifications
    /// Used to track file system mount status changes
    pub mount_signal: Option<mpsc::Sender<bool>>,

    /// Name of the exported file system available to clients
    pub export_name: Arc<String>,

    /// Transaction state tracker for handling retransmissions
    /// Maintains idempotency by detecting duplicate RPC calls
    pub transaction_tracker: Arc<super::TransactionTracker>,

    /// Portmap table storing port-to-program mappings
    /// (like a portmap service)
    pub portmap_table: Arc<RwLock<PortmapTable>>,
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("rpc::Context")
            .field("local_port", &self.local_port)
            .field("client_addr", &self.client_addr)
            .field("auth", &self.auth)
            .finish()
    }
}
