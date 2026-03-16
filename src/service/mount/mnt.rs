//! Service implementation for the MOUNT v3 `MNT` procedure.

use async_trait::async_trait;

use crate::interface::mount::mnt::{Args, Fail, Mnt, Success};

use super::MountService;

#[async_trait]
impl Mnt for MountService {
    async fn mnt(&self, _args: Args) -> Result<Success, Fail> {
        todo!()
    }
}
