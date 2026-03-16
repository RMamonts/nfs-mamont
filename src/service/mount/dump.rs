//! Service implementation for the MOUNT v3 `DUMP` procedure.

use async_trait::async_trait;

use crate::mount::dump::{Dump, Success};

use super::MountService;

#[async_trait]
impl Dump for MountService {
    async fn dump(&self) -> Success {
        todo!()
    }
}
