//! Service implementation for the MOUNT v3 `EXPORT` procedure.

use async_trait::async_trait;

use crate::mount::export::{Export, Success};

use super::MountService;

#[async_trait]
impl Export for MountService {
    async fn export(&self) -> Success {
        let exports = self.exports.by_directory.values().cloned().collect();
        Success { exports }
    }
}
