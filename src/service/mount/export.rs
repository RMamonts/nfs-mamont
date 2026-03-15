use async_trait::async_trait;

use crate::mount::export::{Export, Success};

use super::Service;

#[async_trait]
impl Export for Service {
    async fn export(&self) -> Success {
        todo!()
    }
}
