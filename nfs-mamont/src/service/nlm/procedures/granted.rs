use crate::nlm::procedures::granted::{Granted, Nlm4GrantedRes};
use crate::rpc::auth::Credential;
use crate::service::nlm::NlmService;

impl Granted for NlmService {
    async fn granted(&self, res: Nlm4GrantedRes, _cred: &Credential) {
        todo!();
    }
}
