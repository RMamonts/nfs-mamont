//! Service implementation for the MOUNT v3 `UMNT` procedure.

use crate::mount::umnt::{Args, Umnt};
use crate::rpc::auth::Credential;
use std::net::SocketAddr;

use super::MountService;

impl Umnt for MountService {
    async fn umnt(&self, args: Args, client_addr: SocketAddr, _cred: &Credential) {
        let mut mounts = self.mounts.write().await;
        let client = client_addr.ip().to_canonical();

        if let Some(entries) = mounts.by_client.get_mut(&client) {
            entries.retain(|entry| entry.directory != args.dirpath);
            if entries.is_empty() {
                mounts.by_client.remove(&client);
            }
        }
    }
}
