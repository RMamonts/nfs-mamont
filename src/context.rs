#![allow(dead_code)]

use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{allocator::Impl, vfs};

pub struct ServerContext {
    allocator: Arc<Mutex<Impl>>,
    backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
}

impl ServerContext {
    /// Builds context from allocator limits without exposing allocator implementation.
    pub fn new(
        backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
        buffer_size: NonZeroUsize,
        buffer_count: NonZeroUsize,
    ) -> Self {
        let allocator = Arc::new(Mutex::new(Impl::new(buffer_size, buffer_count)));

        Self { allocator, backend }
    }

    pub fn get_backend(&self) -> Arc<dyn vfs::Vfs + Send + Sync + 'static> {
        Arc::clone(&self.backend)
    }

    pub fn get_allocator(&self) -> Arc<Mutex<Impl>> {
        Arc::clone(&self.allocator)
    }
}
