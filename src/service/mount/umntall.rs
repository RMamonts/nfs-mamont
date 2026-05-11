//! Service implementation for the MOUNT v3 `UMNTALL` procedure.

use std::net::SocketAddr;

use async_trait::async_trait;

use crate::mount::umntall::Umntall;

use super::MountService;

#[async_trait]
impl Umntall for MountService {
    async fn umntall(&self, client_addr: SocketAddr) {
        self.mounts.write().await.by_client.remove(&client_addr);
    }
}
