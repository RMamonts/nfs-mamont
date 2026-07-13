//! Service implementation for the MOUNT v3 `UMNT` procedure.

use std::net::SocketAddr;

use crate::mount::umnt::{Args, Umnt};

use super::MountService;

impl Umnt for MountService {
    async fn umnt(&self, args: Args, client_addr: SocketAddr) {
        let mut inner = self.inner.write().await;

        if let Some(entries) = inner.mounts.by_client.get_mut(&client_addr) {
            entries.retain(|entry| entry.directory != args.dirpath);
            if entries.is_empty() {
                inner.mounts.by_client.remove(&client_addr);
            }
        }
    }
}
