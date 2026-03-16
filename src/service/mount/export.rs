//! Service implementation for the MOUNT v3 `EXPORT` procedure.

use async_trait::async_trait;

use crate::interface::mount::export::{Export, Success};

use super::MountService;

#[async_trait]
impl Export for MountService {
    async fn export(&self) -> Success {
        todo!()
    }
}
