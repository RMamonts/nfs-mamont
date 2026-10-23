use crate::nlm::procedures::lock::{Lock, Nlm4LockArgs, Nlm4LockRes};
use crate::nlm::Nlm4Stats;
use crate::rpc::auth::Credential;

use crate::service::nlm::lock_types;
use crate::service::nlm::NlmService;

impl Lock for NlmService {
    async fn lock(&self, args: Nlm4LockArgs, _cred: &Credential) -> Nlm4LockRes {
        let mut registry = self.locks.write().await;

        let new_lock = lock_types::Lock {
            caller_name: args.lock.caller_name,
            system_identifier: args.lock.system_identifier,
            exclusive: args.exclusive,
            offset: args.lock.lock_offset,
            length: args.lock.lock_length,
            opaque_handle: args.lock.opaque_handle,
            cookie: args.cookie,
        };

        let fh = args.lock.file_handle;
        if args.reclaim || registry.find_conflict(&fh, &new_lock).is_none() {
            if registry.push_or_replace(fh, new_lock).is_err() {
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
