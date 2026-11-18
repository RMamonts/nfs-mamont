//! Service implementation for the MOUNT v3 `DUMP` procedure.

use nfs_mamont::auth::Credential;
use nfs_mamont::mount::dump::{Dump, Success};

use super::MountService;

impl Dump for MountService {
    async fn dump(&self, _cred: &Credential) -> Success {
        let mount_list = self
            .mounts
            .read()
            .await
            .by_client
            .values()
            .flat_map(|entries| entries.iter().cloned())
            .collect();
        Success { mount_list }
    }
}
