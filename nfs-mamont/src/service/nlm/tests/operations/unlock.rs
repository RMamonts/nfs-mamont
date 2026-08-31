use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::nlm::cookie::Cookie;
use crate::nlm::procedures::lock::Lock;
use crate::nlm::procedures::unlock::Unlock;
use crate::nlm::GrantedNotifier;
use crate::nlm::Nlm4Stats;
use crate::service::nlm::tests::{
    fill_fh, make_lock_args_with_block, make_lock_args_without_block, make_unlock_args, FH_DEFAULT,
    LOCK_WHOLE_LENGTH,
};
use crate::service::nlm::NlmService;

fn client_addr(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port)
}

#[tokio::test]
async fn unlock_sends_granted_callback_for_pending_lock() {
    let svc = NlmService::new();
    let addr = client_addr(2049);
    let caller = addr.to_string();

    let (granted_tx, granted_rx) = async_channel::unbounded::<Cookie>();
    svc.add_client(addr, granted_tx).await;

    // Alice holds [0, 100]; Bob blocks on the same range.
    svc.lock(make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, &caller, 100, 1))
        .await;
    let blocked = svc
        .lock(make_lock_args_with_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, &caller, 200, 2))
        .await;
    assert_eq!(blocked.stat, Nlm4Stats::Blocked);

    // Alice unlocks -> freed handle queued; drive the subtask.
    svc.unlock(make_unlock_args(FH_DEFAULT, &caller, 100, 3)).await;
    svc.process(fill_fh(FH_DEFAULT)).await;

    // Bob's pending lock gets granted -> its cookie (2) is delivered to the writetask.
    let cookie = granted_rx.try_recv().expect("expected a GRANTED callback");
    assert_eq!(cookie, Cookie::new(2));
}

#[tokio::test]
async fn unlock_removes_lock_and_allows_new_lock() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0))
        .await;
    svc.unlock(make_unlock_args(FH_DEFAULT, "alice", 100, 1)).await;
    let res = svc
        .lock(make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "bob", 200, 0))
        .await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn unlock_on_nonexistent_lock_returns_granted() {
    let svc = NlmService::new();
    let res = svc.unlock(make_unlock_args(FH_DEFAULT, "nobody", 0, 0)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn unlock_preserves_cookie() {
    let svc = NlmService::new();
    let res = svc.unlock(make_unlock_args(FH_DEFAULT, "nobody", 0, 99)).await;
    assert_eq!(res.cookie.raw(), 99);
}

#[tokio::test]
async fn unlock_auto_grants_pending_exclusive() {
    let svc = NlmService::new();
    // Alice holds [0, 100]
    svc.lock(make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0))
        .await;
    // Bob blocks on the same range
    let blocked = svc
        .lock(make_lock_args_with_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "bob", 200, 0))
        .await;
    assert_eq!(blocked.stat, Nlm4Stats::Blocked);
    // Alice unlocks -> the freed handle is queued for the subtask
    svc.unlock(make_unlock_args(FH_DEFAULT, "alice", 100, 1)).await;
    // Drive the subtask: promote pending locks for the freed file handle
    svc.process(fill_fh(FH_DEFAULT)).await;
    // Charlie should be denied because Bob now holds the lock
    let denied = svc
        .lock(make_lock_args_without_block(
            FH_DEFAULT,
            true,
            0,
            LOCK_WHOLE_LENGTH,
            "charlie",
            300,
            0,
        ))
        .await;
    assert_eq!(denied.stat, Nlm4Stats::Denied);
}
