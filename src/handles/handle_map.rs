use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::handles::inner::Inner;
use crate::vfs;

pub struct HandleMap {
    map: RwLock<Inner>,
    vfs: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
}

impl HandleMap {
    fn new(root: PathBuf, vfs: Arc<dyn vfs::Vfs + Send + Sync + 'static>) -> Self {
        Self { map: RwLock::new(Inner::new(root)), vfs }
    }
}
