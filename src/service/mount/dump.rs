//! Service implementation for the MOUNT v3 `DUMP` procedure.

use async_trait::async_trait;

use crate::mount::dump::{Dump, Success};

use super::Service;

#[async_trait]
impl Dump for Service {
    async fn dump(&self) -> Success {
        todo!()
    }
}
