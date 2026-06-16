use crate::nlm::procedures::unlock::{Nlm4UnlockArgs, Nlm4UnlockRes, Unlock};
use crate::nlm::Nlm4Stats;

use super::{check_caller_name, NlmService};

impl Unlock for NlmService {
    async fn unlock(&self, args: Nlm4UnlockArgs) -> Nlm4UnlockRes {
        if check_caller_name(&args.lock.caller_name).is_err() {
            return Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Failed };
        }

        let mut registry = self.locks.write().await;

        let fh = args.lock.file_handle;
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
        // TODO: Add client notification logic (#267).
        if registry.grant_pending(&fh).is_err() {
            return Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Failed };
        }

        Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Granted }
    }
}
