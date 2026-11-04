//! Service implementation for the MOUNT v3 `UMNTALL` procedure.

use std::net::SocketAddr;

use nfs_mamont::auth::Credential;
use nfs_mamont::mount::umntall::Umntall;

use super::MountService;

impl Umntall for MountService {
    // `UMNTALL` only touches mounts recorded for `client_addr` and has no
    // error code, so the credential adds nothing (see the module docs).
    async fn umntall(&self, client_addr: SocketAddr, _cred: &Credential) {
        self.mounts.write().await.by_client.remove(&client_addr);
    }
}
