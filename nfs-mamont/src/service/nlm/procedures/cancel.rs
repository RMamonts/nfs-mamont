use crate::nlm::procedures::cancel::{Cancel, Nlm4CancelArgs, Nlm4CancelRes};
use crate::nlm::Nlm4Stats;
use crate::rpc::auth::Credential;
use crate::service::nlm::lock_types::Lock;
use crate::service::nlm::NlmService;

impl Cancel for NlmService {
    async fn cancel(&self, args: Nlm4CancelArgs, _cred: &Credential) -> Nlm4CancelRes {
        let target = Lock {
            caller_name: args.lock.caller_name,
            system_identifier: args.lock.system_identifier,
            exclusive: args.exclusive,
            offset: args.lock.lock_offset,
            length: args.lock.lock_length,
            opaque_handle: args.lock.opaque_handle,
            cookie: args.cookie,
        };

        let mut registry = self.locks.write().await;

        let fh = args.lock.file_handle;
        if registry.remove_pending(&fh, &target) {
            return Nlm4CancelRes { cookie: args.cookie, stat: Nlm4Stats::Granted };
        }

        if registry.has_active_lock(&fh, &target) {
            return Nlm4CancelRes { cookie: args.cookie, stat: Nlm4Stats::Granted };
        }

        Nlm4CancelRes { cookie: args.cookie, stat: Nlm4Stats::Denied }
    }
}
