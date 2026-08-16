use crate::nlm::procedures::unlock::{Nlm4UnlockArgs, Nlm4UnlockRes, Unlock};
use crate::nlm::Nlm4Stats;

use super::{check_caller_name, NlmService};

impl Unlock for NlmService {
    async fn unlock(&self, args: Nlm4UnlockArgs) -> Nlm4UnlockRes {
        if check_caller_name(&args.lock.caller_name).is_err() {
            return Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Failed };
        }

        let fh = args.lock.file_handle;

        let granted_locks = {
            let mut registry = self.locks.write().await;

            if registry
                .remove_by_owner(
                    &fh,
                    &args.lock.caller_name,
                    args.lock.system_identifier,
                    args.lock.lock_offset,
                    args.lock.lock_length,
                )
                .is_err()
            {
                return Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Failed };
            }

            match registry.grant_pending(&fh) {
                Ok(granted_locks) => granted_locks,
                Err(_) => return Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Failed },
            }
        };

        for lock in granted_locks {
            if let Some(tx) = lock.grant_notification.granted_tx {
                if let Err(e) = tx.send(lock.grant_notification.cookie).await {
                    tracing::warn!("failed to send grant callback: {}", e);
                }
            }
        }

        Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Granted }
    }
}
