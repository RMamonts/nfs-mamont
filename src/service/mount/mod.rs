//! Server-side state and handlers for the MOUNT v3 RPC program.
//!
//! The structures in this module keep track of two related views:
//! exported directories available for mounting and currently active mounts
//! reported by clients.
//!
//! Mount/filehandle resolution decision:
//! - MOUNT `MNT` returns only initial filehandles for explicitly exported paths;
//! - path traversal inside mounted subtree is expected to go through NFS `LOOKUP`;
//! - therefore MOUNT service owns mapping only for mountable roots, while regular
//!   filename-to-filehandle resolution belongs to VFS/NFS layer;
//! - this mirrors common access policy where clients are granted a specific export
//!   subtree and should not rely on walking to upper directories via MOUNT.
//!
//! State structure follows this split:
//! - exports are keyed by directory path for direct `MNT` lookup;
//! - active mounts are keyed by client socket address because one client can
//!   mount multiple directories and `UMNT`/`UMNTALL` are client-scoped;
//! - each export keeps server policy metadata (file handle + auth flavors)
//!   next to user-visible export data.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use crate::mount::{ExportEntry, MountEntry};
use crate::rpc::AuthFlavor;
use crate::vfs::file;

mod dump;
mod export;
mod mnt;
mod umnt;
mod umntall;

// TODO: should be taken from config
const AUTH: [AuthFlavor; 1] = [AuthFlavor::None];

#[derive(Clone)]
pub struct ExportEntryWrapper {
    pub export: ExportEntry,
    pub root_handle: file::Handle,
}

/// Registry of exported directories advertised by the server
#[derive(Default)]
struct ExportRegistry {
    /// A single directory has at most one export entry
    by_directory: HashMap<file::Path, ExportEntryWrapper>,
}

impl ExportRegistry {
    fn from_entries(entries: Vec<ExportEntryWrapper>) -> Self {
        let mut by_directory = HashMap::new();
        for entry in entries.into_iter() {
            let file_handle = entry.root_handle.clone();
            by_directory.insert(
                entry.export.directory.clone(),
                ExportEntryWrapper { export: entry.export, root_handle: file_handle },
            );
        }
        Self { by_directory }
    }

    fn by_path(&self, path: &file::Path) -> Option<&ExportEntryWrapper> {
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
    pub fn with_exports(entries: Vec<ExportEntryWrapper>) -> Self {
        Self { exports: ExportRegistry::from_entries(entries), mounts: MountRegistry::default() }
    }

    fn export_entry(&self, path: &file::Path) -> Option<&ExportEntryWrapper> {
        self.exports.by_path(path)
    }
}
