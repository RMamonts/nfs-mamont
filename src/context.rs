#![allow(dead_code)]

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{allocator::Impl, vfs};

pub type SharedVfs = Arc<dyn vfs::Vfs + Send + Sync + 'static>;

pub struct ServerContext {
    allocator: Arc<Mutex<Impl>>,
    backend: SharedVfs,
}
