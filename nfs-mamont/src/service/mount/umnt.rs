//! Service implementation for the MOUNT v3 `UMNT` procedure.

use std::net::SocketAddr;

use crate::mount::umnt::{Args, Umnt};

use super::MountService;

impl Umnt for MountService {
    async fn umnt(&self, args: Args, client_addr: SocketAddr) {
        let mut mounts = self.mounts.write().unwrap();

        if let Some(entries) = mounts.by_client.get_mut(&client_addr) {
            entries.retain(|entry| entry.directory != args.dirpath);
            if entries.is_empty() {
                mounts.by_client.remove(&client_addr);
            }
        }
    }
}
