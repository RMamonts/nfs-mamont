//! Service implementation for the MOUNT v3 `UMNTALL` procedure.

use std::net::SocketAddr;

use async_trait::async_trait;

use crate::mount::umntall::Umntall;

use super::MountService;

#[async_trait]
impl Umntall for MountService {
    async fn umntall(&mut self, client_addr: SocketAddr) {
        self.mounts.by_client.remove(&client_addr);
    }
}
