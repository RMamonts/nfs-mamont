//! Service implementation for the MOUNT v3 `UMNTALL` procedure.

use nfs_mamont::auth::Credential;
use nfs_mamont::connection_id::ConnectionId;
use nfs_mamont::mount::umntall::Umntall;

use super::MountService;

impl Umntall for MountService {
    async fn umntall(&self, connection_id: ConnectionId, _cred: &Credential) {
        self.mounts.write().await.by_client.remove(&connection_id);
    }
}
