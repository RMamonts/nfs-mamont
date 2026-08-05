use std::num::NonZeroUsize;
use std::sync::{Arc, RwLock};

use crate::allocator::{Allocator, Buffer};
use crate::task::global::vfs::{SharedBackend, VfsPool};
use crate::vfs;

/// A handle that can attach a VFS backend to a running server from outside
/// the server task.
///
/// Obtain via [`ServerContext::backend_handle`] before moving the context
/// into [`handle_forever`](crate::handle_forever).
///
/// # Example
///
/// ```ignore
/// let context = ServerContext::<_, MyFs, _>::new(…);
/// let backend = context.backend_handle();
/// tokio::spawn(handle_forever(listener, context, …));
/// //   ^^^ context moved — backend handle still usable:
/// backend.attach(Arc::new(MyFs::new(…)));
/// ```
pub struct BackendHandle<V> {
    inner: SharedBackend<V>,
}

impl<V> BackendHandle<V> {
    /// Attaches (or replaces) the VFS backend.
    pub fn attach(&self, backend: Arc<V>) {
        *self.inner.write().unwrap() = Some(backend);
    }
}

/// Shared server resources: VFS worker pool, buffer allocators, and backend.
///
/// Construct once at startup (without a backend) and share across connection
/// handlers. The backend may be attached later via [`add_backend`] (or
/// [`BackendHandle::attach`]) after the server has started.
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
    /// Shared optional filesystem implementation, set after server start.
    shared_backend: SharedBackend<V>,
}

impl<A, V, B> ServerContext<A, V, B>
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    /// Creates a context **without** a backend.
    ///
    /// The server can start accepting connections immediately, but NFS
    /// procedures will return [`vfs::Error::IO`] until a backend is
    /// attached via [`add_backend`].
    pub fn new(
        read_allocator: Arc<A>,
        write_allocator: Arc<A>,
        vfs_pool_size: NonZeroUsize,
    ) -> Self {
        let shared_backend: SharedBackend<V> = Arc::new(RwLock::new(None));
        let vfs_pool =
            VfsPool::new(vfs_pool_size, Arc::clone(&shared_backend), Arc::clone(&read_allocator));

        Self { vfs_pool, read_allocator, write_allocator, shared_backend }
    }

    /// Attaches (or replaces) the VFS backend at runtime.
    ///
    /// All existing and future VFS workers will start using this backend
    /// immediately.
    pub fn add_backend(&self, backend: Arc<V>) {
        *self.shared_backend.write().unwrap() = Some(backend);
    }

    /// Returns a [`BackendHandle`] that survives the move of this context
    /// into [`handle_forever`](crate::handle_forever), allowing the backend
    /// to be attached after the server has started.
    ///
    /// Call **before** moving the context into the server task.
    pub fn backend_handle(&self) -> BackendHandle<V> {
        BackendHandle { inner: Arc::clone(&self.shared_backend) }
    }

    /// Returns the shared VFS worker pool used to dispatch NFS procedure work.
    #[inline]
    pub fn get_vfs_pool(&self) -> &VfsPool<B> {
        &self.vfs_pool
    }

    /// Returns a clone of the [`vfs::Vfs`] backend, if one has been attached.
    #[inline]
    pub fn get_backend(&self) -> Option<Arc<V>> {
        self.shared_backend.read().unwrap().clone()
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
