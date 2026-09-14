//! Service implementation for the MOUNT v3 `UMNTALL` procedure.

use crate::mount::umntall::Umntall;
use crate::rpc::auth::Credential;
use std::net::SocketAddr;

use super::MountService;

impl Umntall for MountService {
    async fn umntall(&self, client_addr: SocketAddr, _cred: &Credential) {
        self.mounts.write().await.by_client.remove(&client_addr);
    }
}
