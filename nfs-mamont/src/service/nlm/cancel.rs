use crate::nlm::procedures::cancel::{Cancel, Nlm4CancelArgs, Nlm4CancelRes};
use crate::nlm::Nlm4Stats;

use super::{NlmService, PendingLock};

impl Cancel for NlmService {
    async fn cancel(&self, args: Nlm4CancelArgs) -> Nlm4CancelRes {
        let cookie = args.cookie;
        let fh_bytes = args.lock.file_handle.0;
        let target = PendingLock {
            caller_name: args.lock.caller_name,
            system_identifier: args.lock.system_identifier,
            exclusive: args.exclusive,
            offset: args.lock.lock_offset,
            length: args.lock.lock_length,
            opaque_handle: args.lock.opaque_handle,
            cookie,
        };

        let mut registry = self.locks.write().await;
        let removed = registry.remove_pending(&fh_bytes, &target);

        match removed {
            true => Nlm4CancelRes { cookie, stat: Nlm4Stats::Granted },
            false => Nlm4CancelRes { cookie, stat: Nlm4Stats::Denied },
        }
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

    use super::super::tests::{fill_fh, fill_opaque, lock_args, lock_args_block};

    fn cancel_args(fh_byte: u8, caller: &str, pid: i32, cookie_val: u64) -> Nlm4CancelArgs {
        Nlm4CancelArgs {
            cookie: Cookie::new(cookie_val),
            block: false,
            exclusive: true,
            lock: Nlm4Lock {
                caller_name: caller.into(),
                file_handle: Handle(fill_fh(fh_byte)),
                opaque_handle: fill_opaque(1),
                system_identifier: pid,
                lock_offset: 0,
                lock_length: 100,
            },
        }
    }

    #[tokio::test]
    async fn cancel_removes_blocked_request() {
        let svc = NlmService::default();
        let svc_ref = &svc;
        svc_ref.lock(lock_args(1, true, 0, 100, "alice", 100)).await;
        let res = svc_ref.lock(lock_args_block(1, true, 0, 100, "bob", 200)).await;
        assert_eq!(res.stat, Nlm4Stats::Blocked);
        let res = svc_ref.cancel(cancel_args(1, "bob", 200, 0)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
        let res = svc_ref.cancel(cancel_args(1, "bob", 200, 0)).await;
        assert_eq!(res.stat, Nlm4Stats::Denied);
    }

    #[tokio::test]
    async fn cancel_on_nonexistent_returns_denied() {
        let svc = NlmService::default();
        let res = svc.cancel(cancel_args(1, "nobody", 0, 0)).await;
        assert_eq!(res.stat, Nlm4Stats::Denied);
    }

    #[tokio::test]
    async fn cancel_preserves_cookie() {
        let svc = NlmService::default();
        let svc_ref = &svc;
        svc_ref.lock(lock_args(1, true, 0, 100, "alice", 100)).await;
        svc_ref.lock(lock_args_block(1, true, 0, 100, "bob", 200)).await;
        let res = svc_ref.cancel(cancel_args(1, "bob", 200, 55)).await;
        assert_eq!(res.cookie.raw(), 55);
    }
}
