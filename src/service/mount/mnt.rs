//! Service implementation for the MOUNT v3 `MNT` procedure.

use async_trait::async_trait;

use crate::mount::mnt::{Args, Fail, Mnt, Success};

use super::Service;

#[async_trait]
impl Mnt for Service {
    async fn mnt(&self, _args: Args) -> Result<Success, Fail> {
        todo!()
    }
}
