use crate::nlm::procedures::test::{Nlm4TestArgs, Nlm4TestReply, Nlm4TestRes, Test};
use crate::nlm::Nlm4Stats;

use super::{ActiveLock, NlmService};

impl Test for NlmService {
    async fn test(&self, args: Nlm4TestArgs) -> Nlm4TestRes {
        let registry = self.locks.read().await;

        let request = match ActiveLock::new(
            args.lock.caller_name,
            args.lock.system_identifier,
            args.exclusive,
            args.lock.lock_offset,
            args.lock.lock_length,
            args.lock.opaque_handle,
        ) {
            Ok(new_lock) => new_lock,
            Err(_) => {
                return Nlm4TestRes {
                    cookie: args.cookie,
                    test_stat: Nlm4TestReply { stat: Nlm4Stats::Failed, holder: None },
                }
            }
        };

        let fh = args.lock.file_handle;
        match registry.find_conflict(&fh, &request) {
            Some(holder) => Nlm4TestRes {
                cookie: args.cookie,
                test_stat: Nlm4TestReply { stat: Nlm4Stats::Denied, holder: Some(holder) },
            },
            None => Nlm4TestRes {
                cookie: args.cookie,
                test_stat: Nlm4TestReply { stat: Nlm4Stats::Granted, holder: None },
            },
        }
    }
}
