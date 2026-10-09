use crate::nlm::procedures::cancel::{Cancel, Nlm4CancelArgs, Nlm4CancelRes};
use crate::nlm::Nlm4Stats;
use crate::rpc::auth::Credential;
use crate::service::nlm::lock_types::{ActiveLock, PendingGrant, PendingLock};
use crate::service::nlm::NlmService;

impl Cancel for NlmService {
    async fn cancel(&self, args: Nlm4CancelArgs, _cred: &Credential) -> Nlm4CancelRes {
        let target = PendingLock {
            caller_name: args.lock.caller_name,
            system_identifier: args.lock.system_identifier,
            exclusive: args.exclusive,
            offset: args.lock.lock_offset,
            length: args.lock.lock_length,
            opaque_handle: args.lock.opaque_handle,
            grant_notification: PendingGrant::new(None, args.cookie),
        };

        let mut registry = self.locks.write().await;

        let fh = args.lock.file_handle;
        if registry.remove_pending(&fh, &target) {
            return Nlm4CancelRes { cookie: args.cookie, stat: Nlm4Stats::Granted };
        }

        let request_as_active: ActiveLock = (&target).into();
        if registry.has_active_lock(&fh, &request_as_active) {
            return Nlm4CancelRes { cookie: args.cookie, stat: Nlm4Stats::Granted };
        }

        Nlm4CancelRes { cookie: args.cookie, stat: Nlm4Stats::Denied }
    }
}
