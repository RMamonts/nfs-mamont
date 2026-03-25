#![allow(dead_code)]

use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::{allocator::Impl, vfs};

pub struct ServerContext<V>
where
    V: vfs::Vfs + Send + Sync + 'static,
{
    read_allocator: Arc<Impl>,
    write_allocator: Arc<Impl>,
    backend: Arc<V>,
}

impl<V> ServerContext<V>
where
    V: vfs::Vfs + Send + Sync + 'static,
{
    /// Builds context from allocator limits without exposing allocator implementation.
    pub fn new(
        backend: Arc<V>,
        read_buffer_size: NonZeroUsize,
        read_buffer_count: NonZeroUsize,
        write_buffer_size: NonZeroUsize,
        write_buffer_count: NonZeroUsize,
    ) -> Self {
        let read_allocator = Arc::new(Impl::new(read_buffer_size, read_buffer_count));
        let write_allocator = Arc::new(Impl::new(write_buffer_size, write_buffer_count));

        Self { read_allocator, write_allocator, backend }
    }

    pub fn get_backend(&self) -> Arc<V> {
        Arc::clone(&self.backend)
    }

    pub fn get_read_allocator(&self) -> Arc<Impl> {
        Arc::clone(&self.read_allocator)
    }

    pub fn get_write_allocator(&self) -> Arc<Impl> {
        Arc::clone(&self.write_allocator)
    }
}
