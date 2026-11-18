//! Service implementation for the MOUNT v3 `UMNT` procedure.

use nfs_mamont::auth::Credential;
use nfs_mamont::connection_id::ConnectionId;
use nfs_mamont::mount::umnt::{Args, Umnt};

use super::MountService;

impl Umnt for MountService {
    async fn umnt(&self, args: Args, connection_id: ConnectionId, _cred: &Credential) {
        let mut mounts = self.mounts.write().await;

        let ip = connection_id.into_inner().ip().to_canonical();
        if let Some(entries) = mounts.by_client.get_mut(&ip) {
            entries.retain(|entry| entry.directory != args.dirpath);
            if entries.is_empty() {
                mounts.by_client.remove(&ip);
            }
        }
    }
}
