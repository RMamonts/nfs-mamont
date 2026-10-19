use crate::auth::Credential;
use crate::nlm::procedures::granted::{Granted, Nlm4GrantedRes};
use crate::service::nlm::NlmService;

impl Granted for NlmService {
    async fn granted(&self, _res: Nlm4GrantedRes, _cred: &Credential) {
        todo!();
    }
}
