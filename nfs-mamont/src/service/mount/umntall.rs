//! Service implementation for the MOUNT v3 `UMNTALL` procedure.

use std::net::SocketAddr;

use crate::mount::umntall::Umntall;

use super::MountService;

impl Umntall for MountService {
    async fn umntall(&self, client_addr: SocketAddr) {
        self.mounts.write().unwrap().by_client.remove(&client_addr);
    }
}
