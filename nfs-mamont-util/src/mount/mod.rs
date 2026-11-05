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
use std::net::IpAddr;

use nfs_mamont::mount::{ExportEntry, MountEntry};
use nfs_mamont::vfs::file;
use nfs_mamont::AuthFlavor;
use tokio::sync::RwLock;

mod dump;
mod export;
mod mnt;
mod umnt;
mod umntall;

#[cfg(test)]
mod tests;

// TODO: should be taken from config
const AUTH: [AuthFlavor; 1] = [AuthFlavor::None];

/// Export entry together with the root file handle returned by `MNT`.
#[derive(Clone)]
struct ExportEntryWrapper {
    export: ExportEntry,
    root_handle: file::Handle,
}

/// Registry of exported directories advertised by the server
#[derive(Default)]
struct ExportRegistry {
    /// A single directory has at most one export entry
    by_directory: HashMap<file::Path, ExportEntryWrapper>,
}

impl ExportRegistry {
    fn by_path(&self, path: &file::Path) -> Option<&ExportEntryWrapper> {
        self.by_directory.get(path)
    }

    fn insert(&mut self, directory: file::Path, root_handle: file::Handle) {
        self.by_directory.insert(
            directory.clone(),
            ExportEntryWrapper {
                export: ExportEntry { directory, names: Vec::new() },
                root_handle,
            },
        );
    }

    fn remove(&mut self, directory: &file::Path) -> bool {
        self.by_directory.remove(directory).is_some()
    }

    fn export_list(&self) -> Vec<ExportEntry> {
        self.by_directory.values().map(|entry| entry.export.clone()).collect()
    }
}

/// Registry of active mounts grouped by client endpoint
#[derive(Default)]
struct MountRegistry {
    /// A single client may mount multiple directories
    by_client: HashMap<IpAddr, HashSet<MountEntry>>,
}

/// In-memory state backing the MOUNT v3 service implementation
pub struct MountService {
    /// Exported directories that are available for mounting
    exports: RwLock<ExportRegistry>,
    /// Active mounts keyed by client.
    mounts: RwLock<MountRegistry>,
}

impl Default for MountService {
    /// Creates a [`MountService`] with no exports and no active mounts.
    fn default() -> Self {
        Self {
            exports: RwLock::new(ExportRegistry::default()),
            mounts: RwLock::new(MountRegistry::default()),
        }
    }
}

impl MountService {
    /// Creates a [`MountService`] with no exports and no active mounts.
    ///
    /// Exports are registered with [`MountService::add_export`].
    pub fn new() -> Self {
        Self::default()
    }

    async fn export_entry(&self, path: &file::Path) -> Option<ExportEntryWrapper> {
        self.exports.read().await.by_path(path).cloned()
    }

    /// Add a new export entry at runtime.
    ///
    /// If an export for `directory` already exists it will be replaced.
    pub async fn add_export(&self, directory: file::Path, root_handle: file::Handle) {
        self.exports.write().await.insert(directory, root_handle);
    }

    /// Remove an export entry at runtime.
    ///
    /// Returns `true` if the export existed and was removed.
    pub async fn remove_export(&self, directory: &file::Path) -> bool {
        self.exports.write().await.remove(directory)
    }
}
