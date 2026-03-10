use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::net::unix::SocketAddr;
use tokio::sync::RwLock;

use crate::rpc::AuthFlavor;
use crate::vfs::{self, file};

pub type SharedVfs = Arc<dyn vfs::Vfs + Send + Sync + 'static>;

pub struct ServerSettings {
    allocator_buffer_size: NonZeroUsize,
    allocator_buffer_count: NonZeroUsize,
}

pub struct ServerExport {
    directory: file::Path,
    allowed_hosts: Vec<String>,
}

pub struct ServerContext {
    settings: ServerSettings,
    backend: Option<SharedVfs>,
    exports: Arc<RwLock<ExportRegistry>>,
    mounts: Arc<RwLock<MountRegistry>>,
}

pub struct ConnectionContext {
    local_addr: Option<SocketAddr>,
    client_addr: Option<SocketAddr>,
    auth: Option<AuthFlavor>,
}

struct ExportRegistry {
    by_directory: HashMap<PathBuf, ServerExport>,
}

struct MountRegistry {
    by_client: HashMap<String, HashMap<PathBuf, file::Path>>,
}
