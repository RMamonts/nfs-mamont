#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::net::unix::SocketAddr;
use tokio::sync::RwLock;

use crate::mount::HostName;
use crate::rpc::AuthFlavor;
use crate::vfs::{self, file};

pub type SharedVfs = Arc<dyn vfs::Vfs + Send + Sync + 'static>;

pub struct ServerSettings {
    allocator_buffer_size: NonZeroUsize,
    allocator_buffer_count: NonZeroUsize,
}

pub struct ServerExport {
    allowed_hosts: Vec<HostName>,
}

pub struct ServerContext {
    settings: ServerSettings,
    backend: SharedVfs,
    exports: Arc<RwLock<ExportRegistry>>,
    mounts: Arc<RwLock<MountRegistry>>,
}

pub struct ConnectionContext {
    local_addr: Option<SocketAddr>,
    client_addr: Option<SocketAddr>,
    auth: Option<AuthFlavor>,
}

struct ExportRegistry {
    by_directory: HashMap<file::Path, ServerExport>,
}

struct MountRegistry {
    by_client: HashMap<SocketAddr, HashSet<file::Path>>,
}
