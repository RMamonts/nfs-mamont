use crate::nlm::procedures::unlock::{Nlm4UnlockArgs, Nlm4UnlockRes, Unlock};
use crate::nlm::Nlm4Stats;

use super::NlmService;

impl Unlock for NlmService {
    async fn unlock(&self, args: Nlm4UnlockArgs) -> Nlm4UnlockRes {
        let fh_bytes = args.lock.file_handle.0;
        let caller_name = args.lock.caller_name;
        let system_identifier = args.lock.system_identifier;

        let mut registry = self.locks.write().await;
        registry.remove_by_owner(
            &fh_bytes,
            &caller_name,
            system_identifier,
            args.lock.lock_offset,
            args.lock.lock_length,
        );

        Nlm4UnlockRes { cookie: args.cookie, stat: Nlm4Stats::Granted }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nlm::cookie::Cookie;
    use crate::nlm::lock::Nlm4Lock;
    use crate::nlm::procedures::lock::Lock;
    use crate::nlm::Nlm4Stats;
    use crate::vfs::file::Handle;

    use super::super::{handle, lock_args, opaque};

    fn unlock_args(fh_byte: u8, caller: &str, pid: i32, cookie_val: u64) -> Nlm4UnlockArgs {
        Nlm4UnlockArgs {
            cookie: Cookie::new(cookie_val),
            lock: Nlm4Lock {
                caller_name: caller.into(),
                file_handle: Handle(handle(fh_byte)),
                opaque_handle: opaque(2),
                system_identifier: pid,
                lock_offset: 0,
                lock_length: 100,
            },
        }
    }

    #[tokio::test]
    async fn unlock_removes_lock_and_allows_new_lock() {
        let svc = super::NlmService::default();
        let svc_ref = &svc;
        svc_ref.lock(lock_args(1, true, 0, 100, "alice", 100)).await;
        svc_ref.unlock(unlock_args(1, "alice", 100, 1)).await;
        let res = svc_ref.lock(lock_args(1, true, 0, 100, "bob", 200)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn unlock_on_nonexistent_lock_returns_granted() {
        let svc = super::NlmService::default();
        let res = svc.unlock(unlock_args(1, "nobody", 0, 0)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn unlock_preserves_cookie() {
        let svc = super::NlmService::default();
        let res = svc.unlock(unlock_args(1, "nobody", 0, 99)).await;
        assert_eq!(res.cookie.raw(), 99);
    }
}
