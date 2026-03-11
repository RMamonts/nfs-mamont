#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::net::unix::SocketAddr;
use tokio::sync::Mutex;

use crate::allocator::Impl;
use crate::mount::{ExportEntry, MountEntry};
use crate::vfs::{self, file};

pub type SharedVfs = Arc<dyn vfs::Vfs + Send + Sync + 'static>;

struct ExportRegistry {
    // one dir can have only one export
    by_directory: HashMap<file::Path, ExportEntry>,
}

struct MountRegistry {
    // one client can mount multiple dirs
    by_client: HashMap<SocketAddr, HashSet<MountEntry>>,
}

pub struct ServerContext {
    allocator: Arc<Mutex<Impl>>,
    backend: SharedVfs,
}
