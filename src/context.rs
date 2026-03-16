#![allow(dead_code)]

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{allocator::Impl, interface::vfs};

pub struct ServerContext {
    allocator: Arc<Mutex<Impl>>,
    backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
}
