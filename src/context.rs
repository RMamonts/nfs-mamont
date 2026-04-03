#![allow(dead_code)]

use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::allocator::Impl;
use crate::task::global::vfs::VfsPool;
use crate::vfs;

/// Shared server resources: VFS worker pool, buffer allocators, and backend.
///
/// Construct once at startup and share across connection handlers.
pub struct ServerContext {
    /// Pool of async workers that execute NFS procedures against [`crate::vfs::Vfs`].
    vfs_pool: VfsPool,
    /// Allocator for read/write buffers (sliced from a pre-sized pool).
    allocator: Arc<Impl>,
    /// Filesystem implementation backing all NFS operations.
    backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
}

impl ServerContext {
    /// Creates a context with the given backend and buffer pool sizes.
    pub fn new(
        backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
        buffer_size: NonZeroUsize,
        buffer_count: NonZeroUsize,
        vfs_pool_size: NonZeroUsize,
    ) -> Self {
        let allocator = Arc::new(Impl::new(buffer_size, buffer_count));
        let vfs_pool = VfsPool::new(vfs_pool_size, Arc::clone(&backend), Arc::clone(&allocator));

        Self { vfs_pool, allocator, backend }
    }

    /// Returns the shared VFS worker pool used to dispatch NFS procedure work.
    pub fn get_vfs_pool(&self) -> &VfsPool {
        &self.vfs_pool
    }

    /// Returns a clone of the [`vfs::Vfs`] backend.
    pub fn get_backend(&self) -> Arc<dyn vfs::Vfs + Send + Sync + 'static> {
        Arc::clone(&self.backend)
    }

    /// Returns a clone of the buffer allocator.
    pub fn get_allocator(&self) -> Arc<Impl> {
        Arc::clone(&self.allocator)
    }
}
