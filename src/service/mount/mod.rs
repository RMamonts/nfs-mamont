//! Server-side state and handlers for the MOUNT v3 RPC program.
//!
//! The structures in this module keep track of two related views:
//! exported directories available for mounting and currently active mounts
//! reported by clients.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use crate::mount::{ExportEntry, MountEntry};
use crate::nfsv3::NFS3_FHSIZE;
use crate::rpc::AuthFlavor;
use crate::vfs::file;

mod dump;
mod export;
mod mnt;
mod umnt;
mod umntall;

#[derive(Clone)]
struct ExportPolicyEntry {
    export: ExportEntry,
    file_handle: file::Handle,
    auth_flavors: Vec<AuthFlavor>,
}

fn stable_export_handle(seed: u64) -> file::Handle {
    let mut bytes = [0u8; NFS3_FHSIZE];
    bytes[..8].copy_from_slice(&seed.to_le_bytes());
    file::Handle(bytes)
}

/// Registry of exported directories advertised by the server
#[derive(Default)]
struct ExportRegistry {
    /// A single directory has at most one export entry
    by_directory: HashMap<file::Path, ExportPolicyEntry>,
}

impl ExportRegistry {
    fn from_entries(entries: Vec<ExportEntry>) -> Self {
        let mut by_directory = HashMap::new();
        for (idx, entry) in entries.into_iter().enumerate() {
            by_directory.insert(
                entry.directory.clone(),
                ExportPolicyEntry {
                    export: entry,
                    file_handle: stable_export_handle((idx + 1) as u64),
                    auth_flavors: vec![AuthFlavor::None],
                },
            );
        }
        Self { by_directory }
    }

    fn by_path(&self, path: &file::Path) -> Option<&ExportPolicyEntry> {
        self.by_directory.get(path)
    }

    fn export_list(&self) -> Vec<ExportEntry> {
        self.by_directory.values().map(|entry| entry.export.clone()).collect()
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
}

impl MountService {
    #[allow(dead_code)]
    pub fn with_exports(entries: Vec<ExportEntry>) -> Self {
        Self { exports: ExportRegistry::from_entries(entries), mounts: MountRegistry::default() }
    }

    fn export_entry(&self, path: &file::Path) -> Option<&ExportPolicyEntry> {
        self.exports.by_path(path)
    }
}
