use super::{check_caller_name, NlmService};
use crate::nlm::procedures::test::Nlm4TestArgs;
use crate::nlm::procedures::unlock::{Nlm4UnlockArgs, Nlm4UnlockRes, Unlock};
use crate::nlm::Nlm4Stats;
use crate::nlm::Nlm4Stats::Granted;
use crate::task::{ProcCall, ProcMessage};

impl Unlock for NlmService {
    async fn unlock(&self, args: Nlm4UnlockArgs) -> Nlm4UnlockRes {
        if check_caller_name(&args.lock.caller_name).is_err() {
            return Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Failed };
        }

        let fh = &args.lock.file_handle;

        let granted_locks = {
            let mut registry = self.locks.write().await;

            if registry
                .remove_by_owner(
                    fh,
                    &args.lock.caller_name,
                    args.lock.system_identifier,
                    args.lock.lock_offset,
                    args.lock.lock_length,
                )
                .is_err()
            {
                return Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Failed };
            }

            match registry.grant_pending(fh) {
                Ok(granted_locks) => granted_locks,
                Err(_) => return Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Failed },
            }
        };

        for lock in granted_locks {
            if let Some(message_sender) = lock.grant_notification.message_sender {
                let test_args = Nlm4TestArgs {
                    cookie: args.cookie,
                    exclusive: lock.exclusive,
                    lock: args.lock.clone(),
                };

                let call = crate::nlm::NlmCall::GRANTED(test_args);
                let proc_message = ProcMessage::Nlm4(call);
                let proc_call = ProcCall { proc_message };
                if let Err(e) = message_sender.send(proc_call).await {
                    tracing::warn!("failed to send grant callback: {}", e);
                }
            }
        }

        Nlm4UnlockRes { cookie: args.cookie, stat: Granted }
    }
}
