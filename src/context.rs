use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::allocator::Allocator;
use crate::task::global::vfs::VfsPool;
use crate::vfs;

/// Shared server resources: VFS worker pool, buffer allocators, and backend.
///
/// Construct once at startup and share across connection handlers.
pub struct ServerContext<A, V>
where
    A: Allocator + Send + Sync + 'static,
    V: vfs::Vfs + Send + Sync + 'static,
{
    /// Pool of async workers that execute NFS procedures against [`crate::vfs::Vfs`].
    vfs_pool: VfsPool,
    /// Allocator for read buffers (sliced from a pre-sized pool).
    read_allocator: Arc<A>,
    /// Allocator for write-side buffers when needed by the stack.
    write_allocator: Arc<A>,
    /// Filesystem implementation backing all NFS operations.
    backend: Arc<V>,
}

impl<A, V> ServerContext<A, V>
where
    A: Allocator + Send + Sync + 'static,
    V: vfs::Vfs + Send + Sync + 'static,
{
    /// Creates a context with the given backend and buffer pool sizes.
    pub fn new(
        backend: Arc<V>,
        read_allocator: A,
        write_allocator: A,
        vfs_pool_size: NonZeroUsize,
    ) -> Self {
        let read_allocator = Arc::new(read_allocator);
        let write_allocator = Arc::new(write_allocator);
        let vfs_pool =
            VfsPool::new(vfs_pool_size, Arc::clone(&backend), Arc::clone(&read_allocator));

        Self { vfs_pool, read_allocator, write_allocator, backend }
    }

    /// Returns the shared VFS worker pool used to dispatch NFS procedure work.
    pub fn get_vfs_pool(&self) -> &VfsPool {
        &self.vfs_pool
    }

    /// Returns a clone of the [`vfs::Vfs`] backend.
    pub fn get_backend(&self) -> Arc<V> {
        Arc::clone(&self.backend)
    }

    /// Returns a clone of the read buffer allocator.
    pub fn get_read_allocator(&self) -> Arc<A> {
        Arc::clone(&self.read_allocator)
    }

    /// Returns a clone of the write buffer allocator.
    pub fn get_write_allocator(&self) -> Arc<A> {
        Arc::clone(&self.write_allocator)
    }
}
