#![allow(dead_code)]

use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::allocator::Impl;
use crate::task::global::vfs::VfsPool;
use crate::vfs::Vfs;

/// Shared server resources: VFS worker pool, buffer allocators, and backend.
///
/// Construct once at startup and share across connection handlers.
pub struct ServerContext<V: Vfs + Send + Sync + 'static> {
    /// Pool of async workers that execute NFS procedures against [`crate::vfs::Vfs`].
    vfs_pool: VfsPool<V>,
    /// Allocator for read buffers (sliced from a pre-sized pool).
    read_allocator: Arc<Impl>,
    /// Allocator for write-side buffers when needed by the stack.
    write_allocator: Arc<Impl>,
    /// Filesystem implementation backing all NFS operations.
    backend: Arc<V>,
}

impl<V: Vfs + Send + Sync + 'static> ServerContext<V> {
    /// Creates a context with the given backend and buffer pool sizes.
    pub fn new(
        backend: Arc<V>,
        read_buffer_size: NonZeroUsize,
        read_buffer_count: NonZeroUsize,
        write_buffer_size: NonZeroUsize,
        write_buffer_count: NonZeroUsize,
        vfs_pool_size: NonZeroUsize,
    ) -> Self {
        let read_allocator = Arc::new(Impl::new(read_buffer_size, read_buffer_count));
        let write_allocator = Arc::new(Impl::new(write_buffer_size, write_buffer_count));
        let vfs_pool =
            VfsPool::new(vfs_pool_size, Arc::clone(&backend), Arc::clone(&read_allocator));

        Self { vfs_pool, read_allocator, write_allocator, backend }
    }

    /// Returns the shared VFS worker pool used to dispatch NFS procedure work.
    pub fn get_vfs_pool(&self) -> &VfsPool<V> {
        &self.vfs_pool
    }

    /// Returns a clone of the backend.
    pub fn get_backend(&self) -> Arc<V> {
        Arc::clone(&self.backend)
    }

    /// Returns a clone of the read buffer allocator.
    pub fn get_read_allocator(&self) -> Arc<Impl> {
        Arc::clone(&self.read_allocator)
    }

    /// Returns a clone of the write buffer allocator.
    pub fn get_write_allocator(&self) -> Arc<Impl> {
        Arc::clone(&self.write_allocator)
    }
}
