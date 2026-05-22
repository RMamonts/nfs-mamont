//! Service implementation for the MOUNT v3 `DUMP` procedure.

use crate::mount::dump::{Dump, Success};

use super::MountService;

impl Dump for MountService {
    async fn dump(&self) -> Success {
        let mount_list = self
            .mounts
            .read()
            .unwrap()
            .by_client
            .values()
            .flat_map(|entries| entries.iter().cloned())
            .collect();
        Success { mount_list }
    }
}
