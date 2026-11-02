//! Service implementation for the MOUNT v3 `EXPORT` procedure.

use nfs_mamont::auth::Credential;
use nfs_mamont::mount::export::{Export, Success};

use super::MountService;

impl Export for MountService {
    async fn export(&self, _cred: &Credential) -> Success {
        let exports = self.exports.read().await.export_list();
        Success { exports }
    }
}
