//! Service implementation for the MOUNT v3 `UMNT` procedure.

use async_trait::async_trait;

use crate::mount::umnt::{Args, Umnt};

use super::MountService;

#[async_trait]
impl Umnt for MountService {
    async fn umnt(&self, _args: Args) {
        todo!()
    }
}
