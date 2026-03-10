#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::net::unix::SocketAddr;
use tokio::sync::RwLock;

use crate::allocator::Allocator;
use crate::mount::{ExportEntry, MountEntry};
use crate::vfs::{self, file};

pub type SharedVfs = Arc<dyn vfs::Vfs + Send + Sync + 'static>;

pub struct ServerSettings {
    allocator_buffer_size: NonZeroUsize,
    allocator_buffer_count: NonZeroUsize,
}

struct ExportRegistry {
    // one dir can have only one export
    by_directory: HashMap<file::Path, ExportEntry>,
}

struct MountRegistry {
    // one client can mount multiple dirs
    by_client: HashMap<SocketAddr, HashSet<MountEntry>>,
}

pub struct ServerContext<T: Allocator> {
    allocator: T,
    backend: SharedVfs,
    // what's available to mount
    exports: Arc<RwLock<ExportRegistry>>,
    // who has mounted what
    mounts: Arc<RwLock<MountRegistry>>,
}
