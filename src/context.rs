#![allow(dead_code)]

use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::allocator::multilevel::alloc::Level;
use crate::allocator::multilevel::constr_two_level;
use crate::vfs;

pub struct ServerContext {
    read_allocator: Arc<Mutex<Level>>,
    write_allocator: Arc<Mutex<Level>>,
    read_buffer_size: NonZeroUsize,
    read_buffer_count: NonZeroUsize,
    write_buffer_size: NonZeroUsize,
    write_buffer_count: NonZeroUsize,
    backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
}

impl ServerContext {
    /// Builds context from allocator limits without exposing allocator implementation.
    pub fn new(
        backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
        read_buffer_size: NonZeroUsize,
        read_buffer_count: NonZeroUsize,
        write_buffer_size: NonZeroUsize,
        write_buffer_count: NonZeroUsize,
    ) -> Self {
        let read_allocator =
            Arc::new(Mutex::new(Level::new(read_buffer_size, read_buffer_count, None)));
        let write_allocator =
            Arc::new(Mutex::new(Level::new(write_buffer_size, write_buffer_count, None)));

        Self {
            read_allocator,
            write_allocator,
            read_buffer_size,
            read_buffer_count,
            write_buffer_size,
            write_buffer_count,
            backend,
        }
    }

    pub fn get_backend(&self) -> Arc<dyn vfs::Vfs + Send + Sync + 'static> {
        Arc::clone(&self.backend)
    }

    pub fn get_read_allocator(&self) -> Level {
        constr_two_level(
            self.read_buffer_size,
            self.read_buffer_count,
            Arc::clone(&self.read_allocator),
        )
    }

    pub fn get_write_allocator(&self) -> Level {
        constr_two_level(
            self.write_buffer_size,
            self.write_buffer_count,
            Arc::clone(&self.write_allocator),
        )
    }
}
