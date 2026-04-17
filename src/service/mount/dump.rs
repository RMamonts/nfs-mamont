//! Service implementation for the MOUNT v3 `DUMP` procedure.

use async_trait::async_trait;

use crate::mount::dump::{Dump, Success};

use super::MountService;

#[async_trait]
impl Dump for MountService {
    async fn dump(&self) -> Success {
        let mount_list =
            self.mounts.by_client.values().flat_map(|entries| entries.iter().cloned()).collect();
        Success { mount_list }
    }
}
