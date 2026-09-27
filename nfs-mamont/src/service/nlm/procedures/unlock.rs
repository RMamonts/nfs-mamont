use crate::auth::Credential;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::test::Nlm4TestArgs;
use crate::nlm::procedures::unlock::{Nlm4UnlockArgs, Nlm4UnlockRes, Unlock};
use crate::nlm::Nlm4Stats;
use crate::nlm::Nlm4Stats::Granted;
use crate::service::nlm::lock_types::{check_caller_name, PendingLock};
use crate::service::nlm::NlmService;
use crate::vfs::file::Handle;

impl Unlock for NlmService {
    async fn unlock(&self, args: Nlm4UnlockArgs, _cred: &Credential) -> Nlm4UnlockRes {
        if check_caller_name(&args.lock.caller_name).is_err() {
            return Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Failed };
        }

        let fh = &args.lock.file_handle;

        let mut granted_locks = {
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

        if let Some(lock) = granted_locks.pop() {
            let mut lock = lock;
            match lock.grant_notification.event_handler.take() {
                Some(handler) => {
                    let test_args = create_test_args(lock, fh.clone());
                    handler.granted(test_args).await;
                }
                None => {
                    tracing::warn!("failed to send grant callback: event_handler is none");
                }
            }
        }

        Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Granted }
    }
}

fn create_test_args(lock: PendingLock, fh: Handle) -> Nlm4TestArgs {
    let alock = Nlm4Lock {
        caller_name: lock.caller_name,
        file_handle: fh,
        lock_length: lock.length,
        lock_offset: lock.offset,
        opaque_handle: lock.opaque_handle,
        system_identifier: lock.system_identifier,
    };
    Nlm4TestArgs { cookie: lock.grant_notification.cookie, exclusive: lock.exclusive, lock: alock }
}
