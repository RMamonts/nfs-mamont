use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::allocator::{Allocator, Buffer};
use crate::task::global::vfs::VfsPool;
use crate::vfs;

/// Shared server resources: VFS worker pool, buffer allocator, and backend.
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
    /// Single allocator serving both READ output buffers and WRITE data buffers,
    /// used on the read side of each connection.
    allocator: Arc<A>,
    /// Filesystem implementation backing all NFS operations.
    backend: Arc<V>,
}

impl<A, V, B> ServerContext<A, V, B>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    /// Creates a context with the given backend and buffer pool size.
    pub fn new(backend: Arc<V>, allocator: Arc<A>, vfs_pool_size: NonZeroUsize) -> Self {
        let vfs_pool = VfsPool::new(vfs_pool_size, Arc::clone(&backend));

        Self { vfs_pool, allocator, backend }
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

    /// Returns a clone of the shared buffer allocator.
    #[inline]
    pub fn get_allocator(&self) -> Arc<A> {
        Arc::clone(&self.allocator)
    }
}
