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

        let fh = args.lock.file_handle;
        let new_active_lock = ActiveLock::from(&new_lock);
        if args.reclaim || registry.find_conflict(&fh, &new_active_lock).is_none() {
            if registry.push_or_replace(fh, new_active_lock).is_err() {
                return Nlm4LockRes { cookie: args.cookie, stat: Nlm4Stats::Failed };
            }

            return Nlm4LockRes { cookie: args.cookie, stat: Nlm4Stats::Granted };
        }

        if !args.block {
            return Nlm4LockRes { cookie: args.cookie, stat: Nlm4Stats::Denied };
        }

        registry.pending.entry(fh).or_default().push(new_lock);

        Nlm4LockRes { cookie: args.cookie, stat: Nlm4Stats::Blocked }
    }
}
