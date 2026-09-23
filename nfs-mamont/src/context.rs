use std::sync::Arc;

use crate::allocator::{Allocator, Buffer};
use crate::task::global::vfs::VfsManager;
use crate::vfs;

/// Shared server resources: vfs manager, buffer allocator, and backend.
///
/// Construct once at startup and share across connection handlers.
pub struct ServerContext<A, V, B>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    /// Pool dispatching NFS procedures against [`crate::vfs::Vfs`] to a single task.
    vfs_manager: VfsManager<B>,
    /// Allocator backing all user-data buffers: READ output buffers and WRITE
    /// payload buffers are served from this single pool.
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
    /// Creates a context with the given backend and allocator.
    pub fn new(backend: Arc<V>, allocator: Arc<A>) -> Self {
        let vfs_manager = VfsManager::new(Arc::clone(&backend));

        Self { vfs_manager, allocator, backend }
    }

    /// Returns the shared vfs manager used to provide link to NFS procedures processor.
    #[inline]
    pub fn get_vfs_manager(&self) -> &VfsManager<B> {
        &self.vfs_manager
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
