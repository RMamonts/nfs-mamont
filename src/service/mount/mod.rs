//! Server-side state and handlers for the MOUNT v3 RPC program.
//!
//! The structures in this module keep track of two related views:
//! exported directories available for mounting and currently active mounts
//! reported by clients.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use crate::mount::{ExportEntry, MountEntry};
use crate::vfs::file;

mod dump;
mod export;
mod mnt;
mod umnt;
mod umntall;

/// Registry of exported directories advertised by the server
#[allow(dead_code)]
#[derive(Default)]
struct ExportRegistry {
    /// A single directory has at most one export entry
    #[allow(dead_code)]
    by_directory: HashMap<file::Path, ExportEntry>,
}

/// Registry of active mounts grouped by client endpoint
#[allow(dead_code)]
#[derive(Default)]
struct MountRegistry {
    /// A single client may mount multiple directories
    #[allow(dead_code)]
    by_client: HashMap<SocketAddr, HashSet<MountEntry>>,
}

/// In-memory state backing the MOUNT v3 service implementation
#[allow(dead_code)]
struct MountService {
    /// Exported directories that are available for mounting
    #[allow(dead_code)]
    exports: ExportRegistry,
    /// Active mounts keyed by client.
    #[allow(dead_code)]
    mounts: MountRegistry,
}
