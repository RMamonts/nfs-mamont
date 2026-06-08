use crate::nlm::procedures::lock::{Lock, Nlm4LockArgs, Nlm4LockRes};
use crate::nlm::Nlm4Stats;

use super::{ActiveLock, NlmService, PendingLock};

impl Lock for NlmService {
    async fn lock(&self, args: Nlm4LockArgs) -> Nlm4LockRes {
        let mut registry = self.locks.write().await;

        let new_lock = match PendingLock::new(
            args.lock.caller_name,
            args.lock.system_identifier,
            args.exclusive,
            args.lock.lock_offset,
            args.lock.lock_length,
            args.lock.opaque_handle,
            args.cookie,
        ) {
            Ok(new_lock) => new_lock,
            Err(_) => return Nlm4LockRes { cookie: args.cookie, stat: Nlm4Stats::Failed },
        };

        let fh_bytes = args.lock.file_handle.0;
        if args.reclaim || registry.find_conflict(&fh_bytes, &ActiveLock::from(&new_lock)).is_none()
        {
            registry.by_file.entry(fh_bytes).or_default().push(ActiveLock::from(&new_lock));

            return Nlm4LockRes { cookie: args.cookie, stat: Nlm4Stats::Granted };
        }

        if !args.block {
            return Nlm4LockRes { cookie: args.cookie, stat: Nlm4Stats::Denied };
        }

        registry.pending.entry(fh_bytes).or_default().push(new_lock);

        Nlm4LockRes { cookie: args.cookie, stat: Nlm4Stats::Blocked }
    }
}
