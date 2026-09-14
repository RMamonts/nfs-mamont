use crate::auth::Credential;

pub mod cancel;
pub mod lock;
pub mod test;
pub mod unlock;

const DEFAULT_CRED: Credential = Credential::None;
