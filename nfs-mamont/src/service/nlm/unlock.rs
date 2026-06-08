use crate::nlm::procedures::unlock::{Nlm4UnlockArgs, Nlm4UnlockRes, Unlock};
use crate::nlm::Nlm4Stats;

use super::{ActiveLock, NlmService};

impl Unlock for NlmService {
    async fn unlock(&self, args: Nlm4UnlockArgs) -> Nlm4UnlockRes {
        let target = match ActiveLock::new(
            args.lock.caller_name,
            args.lock.system_identifier,
            true,
            args.lock.lock_offset,
            args.lock.lock_length,
            args.lock.opaque_handle,
        ) {
            Ok(new_lock) => new_lock,
            Err(_) => return Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Failed },
        };

        let mut registry = self.locks.write().await;

        let fh_bytes = args.lock.file_handle.0;
        registry.remove_by_owner(&fh_bytes, &target);
        registry.grant_pending(&fh_bytes);

        Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Granted }
    }
}
