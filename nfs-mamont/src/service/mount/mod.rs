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

use tokio::sync::RwLock;

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
        for entry in entries {
            by_directory.insert(entry.export.directory.clone(), entry);
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

struct MountServiceInner {
    exports: ExportRegistry,
    mounts: MountRegistry,
}

/// In-memory state backing the MOUNT v3 service implementation
pub struct MountService {
    inner: RwLock<MountServiceInner>,
}

impl MountService {
    pub fn with_exports(entries: Vec<ExportEntryWrapper>) -> Self {
        Self {
            inner: RwLock::new(MountServiceInner {
                exports: ExportRegistry::from_entries(entries),
                mounts: MountRegistry::default(),
            }),
        }
    }

    pub async fn add_export(&self, entry: ExportEntryWrapper) {
        self.inner.write().await.exports.by_directory.insert(entry.export.directory.clone(), entry);
    }

    pub async fn remove_export(&self, path: &file::Path) -> bool {
        self.inner.write().await.exports.by_directory.remove(path).is_some()
    }

    pub async fn export_list(&self) -> Vec<ExportEntry> {
        self.inner.read().await.exports.export_list()
    }

    async fn export_entry(&self, path: &file::Path) -> Option<ExportEntryWrapper> {
        self.inner.read().await.exports.by_path(path).cloned()
    }
}
