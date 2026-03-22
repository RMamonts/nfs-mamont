#![allow(dead_code)]

use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{allocator::Impl, vfs};

pub struct ServerContext {
    allocator_read: Arc<Mutex<Impl>>,
    allocator_write: Arc<Mutex<Impl>>,
    backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
}

impl ServerContext {
    /// Builds context from allocator limits without exposing allocator implementation.
    pub fn new(
        backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
        buffer_size: NonZeroUsize,
        buffer_count: NonZeroUsize,
    ) -> Self {
        let allocator_read = Arc::new(Mutex::new(Impl::new(buffer_size, buffer_count)));
        let allocator_write = Arc::new(Mutex::new(Impl::new(buffer_size, buffer_count)));
        Self { allocator_read, allocator_write, backend }
    }

    pub fn get_backend(&self) -> Arc<dyn vfs::Vfs + Send + Sync + 'static> {
        Arc::clone(&self.backend)
    }

    pub fn get_read_allocator(&self) -> Arc<Mutex<Impl>> {
        Arc::clone(&self.allocator_read)
    }

    pub fn get_write_allocator(&self) -> Arc<Mutex<Impl>> {
        Arc::clone(&self.allocator_write)
    }
}
