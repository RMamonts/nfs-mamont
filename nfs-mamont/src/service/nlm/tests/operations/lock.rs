use super::super::{fill_fh, fill_opaque};
use crate::nlm::cookie::Cookie;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::lock::{Lock, Nlm4LockArgs};
use crate::nlm::procedures::unlock::Unlock;
use crate::nlm::Nlm4Stats;
use crate::service::nlm::NlmService;
use crate::vfs::file::Handle;

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
    let svc = NlmService::new();
    let res = svc.lock(lock_args(1, true, 0, 100, 0)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_denies_conflicting_exclusive() {
    let svc = NlmService::new();
    svc.lock(lock_args(1, true, 0, 100, 0)).await;
    let res = svc.lock(other_args(1, true, 0, 100, 1, false)).await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn lock_allows_shared_overlapping() {
    let svc = NlmService::new();
    svc.lock(lock_args(1, false, 0, 100, 0)).await;
    let res = svc.lock(lock_args(1, false, 10, 20, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_denies_shared_against_exclusive() {
    let svc = NlmService::new();
    svc.lock(lock_args(1, true, 0, 100, 0)).await;
    let res = svc.lock(other_args(1, false, 10, 20, 1, false)).await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn lock_denies_exclusive_against_shared() {
    let svc = NlmService::new();
    svc.lock(lock_args(1, false, 0, 100, 0)).await;
    let res = svc.lock(other_args(1, true, 10, 20, 1, false)).await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn lock_allows_different_files() {
    let svc = NlmService::new();
    svc.lock(lock_args(1, true, 0, 100, 0)).await;
    let res = svc.lock(lock_args(2, true, 0, 100, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_allows_non_overlapping_ranges() {
    let svc = NlmService::new();
    svc.lock(lock_args(1, true, 0, 50, 0)).await;
    let res = svc.lock(lock_args(1, true, 50, 50, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_preserves_cookie() {
    let svc = NlmService::new();
    let res = svc.lock(lock_args(1, true, 0, 100, 42)).await;
    assert_eq!(res.cookie.raw(), 42);
}

#[tokio::test]
async fn lock_blocking_returns_blocked_on_conflict() {
    let svc = NlmService::new();
    svc.lock(lock_args(1, true, 0, 100, 0)).await;
    let res = svc.lock(other_args(1, true, 0, 100, 1, true)).await;
    assert_eq!(res.stat, Nlm4Stats::Blocked);
}

#[tokio::test]
async fn lock_blocking_still_grants_when_free() {
    let svc = NlmService::new();
    let res = svc.lock(lock_args_block(1, true, 0, 100, 42, true)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_non_blocking_still_denies_on_conflict() {
    let svc = NlmService::new();
    svc.lock(lock_args(1, true, 0, 100, 0)).await;
    let res = svc.lock(other_args(1, true, 0, 100, 1, false)).await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn lock_same_owner_re_request_is_granted() {
    let svc = NlmService::new();
    svc.lock(lock_args(1, true, 0, 100, 0)).await;
    let res = svc.lock(lock_args(1, true, 0, 100, 1)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn lock_reclaim_bypasses_conflict_check() {
    let svc = NlmService::new();
    svc.lock(lock_args(1, true, 0, 100, 0)).await;
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
    use crate::nlm::procedures::unlock::Nlm4UnlockArgs;

    let svc = NlmService::new();
    let args = Nlm4LockArgs {
        cookie: Cookie::new(1),
        block: false,
        exclusive: true,
        lock: Nlm4Lock {
            caller_name: "client1".into(),
            file_handle: Handle(fill_fh(1)),
            opaque_handle: fill_opaque(1),
            system_identifier: 100,
            lock_offset: 0,
            lock_length: 100,
        },
        reclaim: false,
        state: 0,
    };
    assert_eq!(svc.lock(args).await.stat, Nlm4Stats::Granted);

    let unlock_args = Nlm4UnlockArgs {
        cookie: Cookie::new(2),
        lock: Nlm4Lock {
            caller_name: "client1".into(),
            file_handle: Handle(fill_fh(1)),
            opaque_handle: fill_opaque(1),
            system_identifier: 100,
            lock_offset: 0,
            lock_length: 100,
        },
    };
    assert_eq!(svc.unlock(unlock_args).await.stat, Nlm4Stats::Granted);

    let relock = Nlm4LockArgs {
        cookie: Cookie::new(3),
        block: false,
        exclusive: true,
        lock: Nlm4Lock {
            caller_name: "client1".into(),
            file_handle: Handle(fill_fh(1)),
            opaque_handle: fill_opaque(1),
            system_identifier: 100,
            lock_offset: 0,
            lock_length: 100,
        },
        reclaim: false,
        state: 0,
    };
    assert_eq!(svc.lock(relock).await.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn multiple_clients_lock_different_ranges_on_same_file() {
    let svc = NlmService::new();
    let args1 = Nlm4LockArgs {
        cookie: Cookie::new(1),
        block: false,
        exclusive: true,
        lock: Nlm4Lock {
            caller_name: "client1".into(),
            file_handle: Handle(fill_fh(1)),
            opaque_handle: fill_opaque(1),
            system_identifier: 100,
            lock_offset: 0,
            lock_length: 50,
        },
        reclaim: false,
        state: 0,
    };
    assert_eq!(svc.lock(args1).await.stat, Nlm4Stats::Granted);

    let args2 = Nlm4LockArgs {
        cookie: Cookie::new(2),
        block: false,
        exclusive: true,
        lock: Nlm4Lock {
            caller_name: "client2".into(),
            file_handle: Handle(fill_fh(1)),
            opaque_handle: fill_opaque(2),
            system_identifier: 200,
            lock_offset: 60,
            lock_length: 50,
        },
        reclaim: false,
        state: 0,
    };
    assert_eq!(svc.lock(args2).await.stat, Nlm4Stats::Granted);
}
