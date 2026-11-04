//! Service implementation for the MOUNT v3 `UMNT` procedure.

use std::net::SocketAddr;

use nfs_mamont::auth::Credential;
use nfs_mamont::mount::umnt::{Args, Umnt};

use super::MountService;

impl Umnt for MountService {
    // `UMNT` only touches mounts recorded for `client_addr` and has no error
    // code, so the credential adds nothing (see the module docs).
    async fn umnt(&self, args: Args, client_addr: SocketAddr, _cred: &Credential) {
        let mut mounts = self.mounts.write().await;

        if let Some(entries) = mounts.by_client.get_mut(&client_addr) {
            entries.retain(|entry| entry.directory != args.dirpath);
            if entries.is_empty() {
                mounts.by_client.remove(&client_addr);
            }
        }
    }
}
