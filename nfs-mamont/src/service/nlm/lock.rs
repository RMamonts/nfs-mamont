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
        if registry.find_conflict(&fh_bytes, &ActiveLock::from(&new_lock)).is_none() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nlm::cookie::Cookie;
    use crate::nlm::lock::Nlm4Lock;
    use crate::vfs::file::Handle;

    use super::super::tests::{fill_fh, fill_opaque};

    fn lock_args(
        fh_byte: u8,
        exclusive: bool,
        offset: u64,
        length: u64,
        cookie_val: u64,
    ) -> Nlm4LockArgs {
        Nlm4LockArgs {
            cookie: Cookie::new(cookie_val),
            block: false,
            exclusive,
            lock: Nlm4Lock {
                caller_name: "test".into(),
                file_handle: Handle(fill_fh(fh_byte)),
                opaque_handle: fill_opaque(1),
                system_identifier: 42,
                lock_offset: offset,
                lock_length: length,
            },
            reclaim: false,
            state: 0,
        }
    }

    fn lock_args_block(
        fh_byte: u8,
        exclusive: bool,
        offset: u64,
        length: u64,
        cookie_val: u64,
        block: bool,
    ) -> Nlm4LockArgs {
        Nlm4LockArgs {
            cookie: Cookie::new(cookie_val),
            block,
            exclusive,
            lock: Nlm4Lock {
                caller_name: "test".into(),
                file_handle: Handle(fill_fh(fh_byte)),
                opaque_handle: fill_opaque(1),
                system_identifier: 42,
                lock_offset: offset,
                lock_length: length,
            },
            reclaim: false,
            state: 0,
        }
    }

    fn other_args(
        fh_byte: u8,
        exclusive: bool,
        offset: u64,
        length: u64,
        cookie_val: u64,
        block: bool,
    ) -> Nlm4LockArgs {
        Nlm4LockArgs {
            cookie: Cookie::new(cookie_val),
            block,
            exclusive,
            lock: Nlm4Lock {
                caller_name: "other".into(),
                file_handle: Handle(fill_fh(fh_byte)),
                opaque_handle: fill_opaque(2),
                system_identifier: 99,
                lock_offset: offset,
                lock_length: length,
            },
            reclaim: false,
            state: 0,
        }
    }

    #[tokio::test]
    async fn lock_grants_exclusive_lock() {
        let svc = NlmService::default();
        let res = svc.lock(lock_args(1, true, 0, 100, 0)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn lock_denies_conflicting_exclusive() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, true, 0, 100, 0)).await;
        let res = svc.lock(other_args(1, true, 0, 100, 1, false)).await;
        assert_eq!(res.stat, Nlm4Stats::Denied);
    }

    #[tokio::test]
    async fn lock_allows_shared_overlapping() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, false, 0, 100, 0)).await;
        let res = svc.lock(lock_args(1, false, 10, 20, 1)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn lock_denies_shared_against_exclusive() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, true, 0, 100, 0)).await;
        let res = svc.lock(other_args(1, false, 10, 20, 1, false)).await;
        assert_eq!(res.stat, Nlm4Stats::Denied);
    }

    #[tokio::test]
    async fn lock_denies_exclusive_against_shared() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, false, 0, 100, 0)).await;
        let res = svc.lock(other_args(1, true, 10, 20, 1, false)).await;
        assert_eq!(res.stat, Nlm4Stats::Denied);
    }

    #[tokio::test]
    async fn lock_allows_different_files() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, true, 0, 100, 0)).await;
        let res = svc.lock(lock_args(2, true, 0, 100, 1)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn lock_allows_non_overlapping_ranges() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, true, 0, 50, 0)).await;
        let res = svc.lock(lock_args(1, true, 50, 50, 1)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn lock_preserves_cookie() {
        let svc = NlmService::default();
        let res = svc.lock(lock_args(1, true, 0, 100, 42)).await;
        assert_eq!(res.cookie.raw(), 42);
    }

    #[tokio::test]
    async fn lock_blocking_returns_blocked_on_conflict() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, true, 0, 100, 0)).await;
        let res = svc.lock(other_args(1, true, 0, 100, 1, true)).await;
        assert_eq!(res.stat, Nlm4Stats::Blocked);
    }

    #[tokio::test]
    async fn lock_blocking_still_grants_when_free() {
        let svc = NlmService::default();
        let res = svc.lock(lock_args_block(1, true, 0, 100, 42, true)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn lock_non_blocking_still_denies_on_conflict() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, true, 0, 100, 0)).await;
        let res = svc.lock(other_args(1, true, 0, 100, 1, false)).await;
        assert_eq!(res.stat, Nlm4Stats::Denied);
    }

    #[tokio::test]
    async fn lock_same_owner_re_request_is_granted() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, true, 0, 100, 0)).await;
        let res = svc.lock(lock_args(1, true, 0, 100, 1)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }
}
