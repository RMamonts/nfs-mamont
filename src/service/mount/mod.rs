//! Server-side state and handlers for the MOUNT v3 RPC program.
//!
//! The structures in this module keep track of two related views:
//! exported directories available for mounting and currently active mounts
//! reported by clients.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use crate::context::SharedVfs;
use crate::mount::{ExportEntry, MountEntry};
use crate::vfs::file;

mod dump;
mod export;
mod mnt;
mod umnt;
mod umntall;

/// Registry of exported directories advertised by the server
#[derive(Default)]
struct ExportRegistry {
    /// A single directory has at most one export entry
    by_directory: HashMap<file::Path, ExportEntry>,
}

impl ExportRegistry {
    fn from_entries(entries: Vec<ExportEntry>) -> Self {
        let mut by_directory = HashMap::new();
        for entry in entries {
            by_directory.insert(entry.directory.clone(), entry);
        }
        Self { by_directory }
    }
}

/// Registry of active mounts grouped by client endpoint
#[derive(Default)]
struct MountRegistry {
    /// A single client may mount multiple directories
    by_client: HashMap<SocketAddr, HashSet<MountEntry>>,
}

/// In-memory state backing the MOUNT v3 service implementation
pub struct MountService {
    /// Exported directories that are available for mounting
    exports: ExportRegistry,
    /// Active mounts keyed by client.
    mounts: MountRegistry,

    vfs: SharedVfs,
}

impl MountService {
    #[allow(dead_code)]
    pub fn with_exports(entries: Vec<ExportEntry>, vfs: SharedVfs) -> Self {
        Self {
            exports: ExportRegistry::from_entries(entries),
            mounts: MountRegistry::default(),
            vfs,
        }
    }
}
