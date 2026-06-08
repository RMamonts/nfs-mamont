use super::super::{fill_fh, fill_opaque, lock_args, lock_args_block};
use crate::nlm::cookie::Cookie;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::lock::Lock;
use crate::nlm::procedures::unlock::{Nlm4UnlockArgs, Unlock};
use crate::nlm::Nlm4Stats;
use crate::service::nlm::NlmService;
use crate::vfs::file::Handle;

fn unlock_args(fh_byte: u8, caller: &str, pid: i32, cookie_val: u64) -> Nlm4UnlockArgs {
    Nlm4UnlockArgs {
        cookie: Cookie::new(cookie_val),
        lock: Nlm4Lock {
            caller_name: caller.into(),
            file_handle: Handle(fill_fh(fh_byte)),
            opaque_handle: fill_opaque(2),
            system_identifier: pid,
            lock_offset: 0,
            lock_length: 100,
        },
    }
}

#[tokio::test]
async fn unlock_removes_lock_and_allows_new_lock() {
    let svc = NlmService::new();
    svc.lock(lock_args(1, true, 0, 100, "alice", 100)).await;
    svc.unlock(unlock_args(1, "alice", 100, 1)).await;
    let res = svc.lock(lock_args(1, true, 0, 100, "bob", 200)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn unlock_on_nonexistent_lock_returns_granted() {
    let svc = NlmService::new();
    let res = svc.unlock(unlock_args(1, "nobody", 0, 0)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn unlock_preserves_cookie() {
    let svc = NlmService::new();
    let res = svc.unlock(unlock_args(1, "nobody", 0, 99)).await;
    assert_eq!(res.cookie.raw(), 99);
}

#[tokio::test]
async fn unlock_auto_grants_pending_exclusive() {
    let svc = NlmService::new();
    // Alice holds [0, 100]
    svc.lock(lock_args(1, true, 0, 100, "alice", 100)).await;
    // Bob blocks on the same range
    let blocked = svc.lock(lock_args_block(1, true, 0, 100, "bob", 200)).await;
    assert_eq!(blocked.stat, Nlm4Stats::Blocked);
    // Alice unlocks -> Bob should be auto-granted
    svc.unlock(unlock_args(1, "alice", 100, 1)).await;
    // Charlie should be denied because Bob now holds the lock
    let denied = svc.lock(lock_args(1, true, 0, 100, "charlie", 300)).await;
    assert_eq!(denied.stat, Nlm4Stats::Denied);
}
