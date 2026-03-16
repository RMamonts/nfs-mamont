use async_trait::async_trait;

use super::{file, Error};

#[async_trait]
pub trait Utils {
    async fn path_to_handle(&self, path: &file::Path) -> Result<file::Handle, Error>;
}
