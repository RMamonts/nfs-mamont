//! Service implementation for the MOUNT v3 `EXPORT` procedure.

use crate::auth::Credential;
use crate::mount::export::{Export, Success};

use super::MountService;

impl Export for MountService {
    async fn export(&self, _cred: &Credential) -> Success {
        let exports = self.exports.export_list();
        Success { exports }
    }
}
