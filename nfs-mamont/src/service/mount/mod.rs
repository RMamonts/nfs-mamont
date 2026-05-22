use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::RwLock;

use crate::mount::{ExportEntry, MountEntry};
use crate::rpc::AuthFlavor;
use crate::vfs::file;

mod dump;
mod export;
mod mnt;
mod umnt;
mod umntall;

const AUTH: [AuthFlavor; 1] = [AuthFlavor::None];

#[derive(Clone)]
pub struct ExportEntryWrapper {
    pub export: ExportEntry,
    pub root_handle: file::Handle,
}

#[derive(Default)]
struct ExportRegistry {
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

#[derive(Default)]
struct MountRegistry {
    by_client: HashMap<SocketAddr, HashSet<MountEntry>>,
}

pub struct MountService {
    exports: Arc<ExportRegistry>,
    mounts: RwLock<MountRegistry>,
}

impl MountService {
    pub fn with_exports(entries: Vec<ExportEntryWrapper>) -> Self {
        Self {
            exports: Arc::new(ExportRegistry::from_entries(entries)),
            mounts: RwLock::new(MountRegistry::default()),
        }
    }

    async fn export_entry(&self, path: &file::Path) -> Option<&ExportEntryWrapper> {
        self.exports.by_path(path)
    }
}
