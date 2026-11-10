//! Service implementation for the MOUNT v3 `UMNTALL` procedure.

use nfs_mamont::auth::Credential;
use nfs_mamont::client_id::ConnectionInfo;
use nfs_mamont::mount::umntall::Umntall;

use super::MountService;

impl Umntall for MountService {
    async fn umntall(&self, connection: ConnectionInfo, _cred: &Credential) {
        self.mounts.write().await.by_client.remove(&connection.peer_addr.ip().to_canonical());
    }
}
