use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::allocator::{Allocator, Buffer};
use crate::task::global::vfs::VfsPool;
use crate::vfs;

/// Shared server resources: VFS worker pool, buffer allocators, and backend.
///
/// Construct once at startup and share across connection handlers.
pub struct ServerContext<A, V, B>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    /// Pool of async workers that execute NFS procedures against [`crate::vfs::Vfs`].
    vfs_pool: VfsPool<B>,
    /// Allocator for read buffers (sliced from a pre-sized pool).
    read_allocator: Arc<A>,
    /// Allocator for write-side buffers when needed by the stack.
    write_allocator: Arc<A>,
    /// Filesystem implementation backing all NFS operations.
    backend: Arc<V>,
}

impl<A, V, B> ServerContext<A, V, B>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    /// Creates a context with the given backend and buffer pool sizes.
    pub fn new(
        backend: Arc<V>,
        read_allocator: Arc<A>,
        write_allocator: Arc<A>,
        vfs_pool_size: NonZeroUsize,
    ) -> Self {
        let vfs_pool =
            VfsPool::new(vfs_pool_size, Arc::clone(&backend), Arc::clone(&read_allocator));

        Self { vfs_pool, read_allocator, write_allocator, backend }
    }

    /// Returns the shared VFS worker pool used to dispatch NFS procedure work.
    #[inline]
    pub fn get_vfs_pool(&self) -> &VfsPool<B> {
        &self.vfs_pool
    }

    /// Returns a clone of the [`vfs::Vfs`] backend.
    #[inline]
    pub fn get_backend(&self) -> Arc<V> {
        Arc::clone(&self.backend)
    }

    /// Returns a clone of the read buffer allocator.
    #[inline]
    pub fn get_read_allocator(&self) -> Arc<A> {
        Arc::clone(&self.read_allocator)
    }

    /// Returns a clone of the write buffer allocator.
    #[inline]
    pub fn get_write_allocator(&self) -> Arc<A> {
        Arc::clone(&self.write_allocator)
    }
}
