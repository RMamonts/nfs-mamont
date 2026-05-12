//! Service implementation for the MOUNT v3 `EXPORT` procedure.

use crate::mount::export::{Export, Success};

use super::MountService;

impl Export for MountService {
    async fn export(&self) -> Success {
        let exports = self.exports.export_list();
        Success { exports }
    }
}
