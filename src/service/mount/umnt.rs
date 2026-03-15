use async_trait::async_trait;

use crate::mount::umnt::{Args, Umnt};

use super::Service;

#[async_trait]
impl Umnt for Service {
    async fn umnt(&self, _args: Args) {
        todo!()
    }
}
