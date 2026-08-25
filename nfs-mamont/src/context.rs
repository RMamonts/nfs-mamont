use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::allocator::{Allocator, Buffer};
use crate::backend::BackendRegistry;
use crate::task::global::vfs::VfsPool;
use crate::vfs;
use crate::vfs::file::BackendId;

/// Shared server resources: VFS worker pool, buffer allocators, and backends.
///
/// Construct once at startup and share across connection handlers. The context is
/// created **without** backends: they are attached at runtime with [`Self::add_backend`]
/// and detached with [`Self::remove_backend`]. Procedures whose file handle points at a
/// backend that is not attached fail with [`vfs::Error::StaleFile`].
///
/// # Example
///
/// ```ignore
/// let context = ServerContext::new(read_allocator, write_allocator, vfs_pool_size);
/// // Keep a registry handle before the context is moved into the server task.
/// let backends = context.backends();
/// tokio::spawn(handle_forever(listener, context, mount_service, nlm_service));
///
/// let id = backends.add(Arc::new(MyFs::new())).expect("no free backend slot");
/// backends.remove(id);
/// ```
pub struct ServerContext<AR, AW, V, BR, BW>
where
    AR: Allocator<Buffer = BR> + Send + Sync + 'static,
    BR: Buffer + 'static,
    AW: Allocator<Buffer = BW> + Send + Sync + 'static,
    BW: Buffer + 'static,
    V: vfs::Vfs<BR, BW> + Send + Sync + 'static,
{
    /// Pool of async workers that execute NFS procedures against [`crate::vfs::Vfs`].
    vfs_pool: VfsPool<BR, BW>,
    /// Allocator for read buffers (sliced from a pre-sized pool).
    read_allocator: Arc<AR>,
    /// Allocator for write-side buffers when needed by the stack.
    write_allocator: Arc<AW>,
    /// Filesystem implementations backing NFS operations, keyed by backend index.
    backends: BackendRegistry<V>,
}

impl<AR, AW, V, BR, BW> ServerContext<AR, AW, V, BR, BW>
where
    AR: Allocator<Buffer = BR> + Send + Sync + 'static,
    BR: Buffer + 'static,
    AW: Allocator<Buffer = BW> + Send + Sync + 'static,
    BW: Buffer + 'static,
    V: vfs::Vfs<BR, BW> + Send + Sync + 'static,
{
    /// Creates a context with the given buffer pool sizes and no backends attached.
    pub fn new(
        read_allocator: Arc<AR>,
        write_allocator: Arc<AW>,
        vfs_pool_size: NonZeroUsize,
    ) -> Self {
        let backends = BackendRegistry::new();
        let vfs_pool = VfsPool::new(vfs_pool_size, backends.clone(), Arc::clone(&read_allocator));

        Self { vfs_pool, read_allocator, write_allocator, backends }
    }

    /// Attaches `backend` at runtime, making it serve requests immediately.
    ///
    /// # Returns
    ///
    /// The index the backend was registered under --- the same index the backend must
    /// encode into the file handles it returns --- or [`None`] if no free slot is left.
    pub fn add_backend(&self, backend: Arc<V>) -> Option<BackendId> {
        self.backends.add(backend)
    }

    /// Detaches the backend registered under `id`.
    ///
    /// # Returns
    ///
    /// The detached backend, or [`None`] if `id` was not registered.
    pub fn remove_backend(&self, id: BackendId) -> Option<Arc<V>> {
        self.backends.remove(id)
    }

    /// Returns the shared VFS worker pool used to dispatch NFS procedure work.
    #[inline]
    pub fn get_vfs_pool(&self) -> &VfsPool<BR, BW> {
        &self.vfs_pool
    }

    /// Returns a clone of the [`vfs::Vfs`] backend registered under `id`, if any.
    #[inline]
    pub fn get_backend(&self, id: BackendId) -> Option<Arc<V>> {
        self.backends.get(id)
    }

    /// Returns a clone of the backend registry.
    ///
    /// The clone shares the same map, so it can be used to attach and detach backends
    /// after the context has been moved into [`crate::handle_forever`].
    #[inline]
    pub fn backends(&self) -> BackendRegistry<V> {
        self.backends.clone()
    }

    /// Returns a clone of the read buffer allocator.
    #[inline]
    pub fn get_read_allocator(&self) -> Arc<AR> {
        Arc::clone(&self.read_allocator)
    }

    /// Returns a clone of the write buffer allocator.
    #[inline]
    pub fn get_write_allocator(&self) -> Arc<AW> {
        Arc::clone(&self.write_allocator)
    }
}
