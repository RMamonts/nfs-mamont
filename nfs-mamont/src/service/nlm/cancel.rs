use super::{ActiveLock, NlmService, PendingLock};
use crate::nlm::procedures::cancel::{Cancel, Nlm4CancelArgs, Nlm4CancelRes};
use crate::nlm::Nlm4Stats;

impl Cancel for NlmService {
    async fn cancel(&self, args: Nlm4CancelArgs) -> Nlm4CancelRes {
        let target = match PendingLock::new(
            args.lock.caller_name,
            args.lock.system_identifier,
            args.exclusive,
            args.lock.lock_offset,
            args.lock.lock_length,
            args.lock.opaque_handle,
            args.cookie,
        ) {
            Ok(new_lock) => new_lock,
            Err(_) => return Nlm4CancelRes { cookie: args.cookie, stat: Nlm4Stats::Failed },
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
