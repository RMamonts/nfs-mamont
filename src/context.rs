#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::net::unix::SocketAddr;
use tokio::sync::RwLock;

use crate::allocator::MemoryBudget;
use crate::mount::{ExportEntry, MountEntry};
use crate::vfs::{self, file};

pub type SharedVfs = Arc<dyn vfs::Vfs + Send + Sync + 'static>;

#[derive(Clone, Copy)]
pub struct AllocatorConfig {
    pub buffer_size: NonZeroUsize,
    pub local_buffer_count: NonZeroUsize,
    pub global_buffer_budget: NonZeroUsize,
}

impl AllocatorConfig {
    pub const fn new(
        buffer_size: NonZeroUsize,
        local_buffer_count: NonZeroUsize,
        global_buffer_budget: NonZeroUsize,
    ) -> Self {
        Self { buffer_size, local_buffer_count, global_buffer_budget }
    }
}

struct ExportRegistry {
    // one dir can have only one export
    by_directory: HashMap<file::Path, ExportEntry>,
}

struct MountRegistry {
    // one client can mount multiple dirs
    by_client: HashMap<SocketAddr, HashSet<MountEntry>>,
}

pub struct ServerContext {
    allocator_config: AllocatorConfig,
    allocator_budget: MemoryBudget,
    backend: SharedVfs,
    // what's available to mount
    exports: Arc<RwLock<ExportRegistry>>,
    // who has mounted what
    mounts: Arc<RwLock<MountRegistry>>,
}

impl ServerContext {
    pub fn new(backend: SharedVfs, allocator_config: AllocatorConfig) -> Self {
        Self {
            allocator_budget: MemoryBudget::new(allocator_config.global_buffer_budget),
            allocator_config,
            backend,
            exports: Arc::new(RwLock::new(ExportRegistry { by_directory: HashMap::new() })),
            mounts: Arc::new(RwLock::new(MountRegistry { by_client: HashMap::new() })),
        }
    }

    pub fn allocator_config(&self) -> AllocatorConfig {
        self.allocator_config
    }

    pub fn allocator_budget(&self) -> MemoryBudget {
        self.allocator_budget.clone()
    }
}
