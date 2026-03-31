#![allow(dead_code)]

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::handles::HandleMap;
use crate::{allocator::Impl, vfs};

pub struct ServerContext {
    read_allocator: Arc<Mutex<Impl>>,
    write_allocator: Arc<Mutex<Impl>>,
    backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
    handles: Arc<HandleMap>,
}

impl ServerContext {
    /// Builds context from allocator limits without exposing allocator implementation.
    pub fn new(
        backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
        root: PathBuf,
        read_buffer_size: NonZeroUsize,
        read_buffer_count: NonZeroUsize,
        write_buffer_size: NonZeroUsize,
        write_buffer_count: NonZeroUsize,
    ) -> Self {
        let read_allocator = Arc::new(Mutex::new(Impl::new(read_buffer_size, read_buffer_count)));
        let write_allocator =
            Arc::new(Mutex::new(Impl::new(write_buffer_size, write_buffer_count)));
        let handles = Arc::new(HandleMap::new(root));
        Self { read_allocator, write_allocator, backend, handles }
    }

    pub fn get_backend(&self) -> Arc<dyn vfs::Vfs + Send + Sync + 'static> {
        Arc::clone(&self.backend)
    }

    pub fn get_read_allocator(&self) -> Arc<Mutex<Impl>> {
        Arc::clone(&self.read_allocator)
    }

    pub fn get_write_allocator(&self) -> Arc<Mutex<Impl>> {
        Arc::clone(&self.write_allocator)
    }

    pub fn get_handle_map(&self) -> Arc<HandleMap> {
        Arc::clone(&self.handles)
    }
}
