use super::super::{
    fill_fh, fill_opaque, make_lock_args_with_block, make_lock_args_without_block, make_unlock_args,
};
use crate::nlm::cookie::Cookie;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::lock::{Lock, Nlm4LockArgs};
use crate::nlm::procedures::unlock::Unlock;
use crate::nlm::Nlm4Stats;
use crate::service::nlm::NlmService;
use crate::vfs::file::Handle;

#[tokio::test]
async fn lock_grants_exclusive_lock() {
    let svc = NlmService::new();
    let res = svc.lock(make_lock_args_without_block(1, true, 0, 100, "test", 42, 0)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_denies_conflicting_exclusive() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(1, true, 0, 100, "test", 42, 0)).await;
    let res = svc.lock(make_lock_args_without_block(1, true, 0, 100, "other", 99, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn lock_allows_shared_overlapping() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(1, false, 0, 100, "test", 42, 0)).await;
    let res = svc.lock(make_lock_args_without_block(1, false, 10, 20, "test", 42, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_denies_shared_against_exclusive() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(1, true, 0, 100, "test", 42, 0)).await;
    let res = svc.lock(make_lock_args_without_block(1, false, 10, 20, "other", 99, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn lock_denies_exclusive_against_shared() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(1, false, 0, 100, "test", 42, 0)).await;
    let res = svc.lock(make_lock_args_without_block(1, true, 10, 20, "other", 99, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn lock_allows_different_files() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(1, true, 0, 100, "test", 42, 0)).await;
    let res = svc.lock(make_lock_args_without_block(2, true, 0, 100, "test", 42, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_allows_non_overlapping_ranges() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(1, true, 0, 50, "test", 42, 0)).await;
    let res = svc.lock(make_lock_args_without_block(1, true, 50, 50, "test", 42, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_preserves_cookie() {
    let svc = NlmService::new();
    let res = svc.lock(make_lock_args_without_block(1, true, 0, 100, "test", 42, 42)).await;
    assert_eq!(res.cookie.raw(), 42);
}

#[tokio::test]
async fn lock_blocking_returns_blocked_on_conflict() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(1, true, 0, 100, "test", 42, 0)).await;
    let res = svc.lock(make_lock_args_with_block(1, true, 0, 100, "other", 99, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Blocked);
}

#[tokio::test]
async fn lock_blocking_still_grants_when_free() {
    let svc = NlmService::new();
    let res = svc.lock(make_lock_args_with_block(1, true, 0, 100, "test", 42, 42)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_non_blocking_still_denies_on_conflict() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(1, true, 0, 100, "test", 42, 0)).await;
    let res = svc.lock(make_lock_args_without_block(1, true, 0, 100, "other", 99, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn lock_same_owner_re_request_is_granted() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(1, true, 0, 100, "test", 42, 0)).await;
    let res = svc.lock(make_lock_args_without_block(1, true, 0, 100, "test", 42, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_reclaim_bypasses_conflict_check() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(1, true, 0, 100, "test", 42, 0)).await;
    let res = svc
        .lock(Nlm4LockArgs {
            cookie: Cookie::new(1),
            block: false,
            exclusive: true,
            lock: Nlm4Lock {
                caller_name: "other".into(),
                file_handle: Handle(fill_fh(1)),
                opaque_handle: fill_opaque(2),
                system_identifier: 99,
                lock_offset: 0,
                lock_length: 100,
            },
            reclaim: true,
            state: 0,
        })
        .await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_unlock_lock_sequence_same_client() {
    let svc = NlmService::new();
    assert_eq!(
        svc.lock(make_lock_args_without_block(1, true, 0, 100, "client1", 100, 1)).await.stat,
        Nlm4Stats::Granted
    );
    assert_eq!(svc.unlock(make_unlock_args(1, "client1", 100, 2)).await.stat, Nlm4Stats::Granted);
    assert_eq!(
        svc.lock(make_lock_args_without_block(1, true, 0, 100, "client1", 100, 3)).await.stat,
        Nlm4Stats::Granted
    );
}

#[tokio::test]
async fn multiple_clients_lock_different_ranges_on_same_file() {
    let svc = NlmService::new();
    assert_eq!(
        svc.lock(make_lock_args_without_block(1, true, 0, 50, "client1", 100, 1)).await.stat,
        Nlm4Stats::Granted
    );
    assert_eq!(
        svc.lock(make_lock_args_without_block(1, true, 60, 50, "client2", 200, 2)).await.stat,
        Nlm4Stats::Granted
    );
}
