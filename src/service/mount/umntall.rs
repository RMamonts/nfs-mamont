//! Service implementation for the MOUNT v3 `UMNTALL` procedure.

use async_trait::async_trait;

use crate::mount::umntall::Umntall;

use super::Service;

#[async_trait]
impl Umntall for Service {
    async fn umntall(&self) {
        todo!()
    }
}
