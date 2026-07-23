use crate::nlm::cookie::Cookie;
use crate::nlm::procedures::lock::Lock;
use crate::nlm::procedures::unlock::Unlock;
use crate::nlm::Nlm4Stats;
use crate::service::nlm::tests::{
    make_lock_args_with_block, make_lock_args_without_block, make_unlock_args, FH_DEFAULT,
    LOCK_WHOLE_LENGTH,
};
use crate::service::nlm::NlmService;

#[tokio::test]
async fn unlock_removes_lock_and_allows_new_lock() {
    let svc = NlmService::new();
    svc.lock(
        None,
        make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0),
    )
    .await;
    svc.unlock(make_unlock_args(FH_DEFAULT, "alice", 100, 1)).await;
    let res = svc
        .lock(
            None,
            make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "bob", 200, 0),
        )
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
    svc.lock(
        None,
        make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0),
    )
    .await;
    // Bob blocks on the same range
    let blocked = svc
        .lock(
            None,
            make_lock_args_with_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "bob", 200, 0),
        )
        .await;
    assert_eq!(blocked.stat, Nlm4Stats::Blocked);
    // Alice unlocks -> Bob should be auto-granted
    svc.unlock(make_unlock_args(FH_DEFAULT, "alice", 100, 1)).await;
    // Charlie should be denied because Bob now holds the lock
    let denied = svc
        .lock(
            None,
            make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "charlie", 300, 0),
        )
        .await;
    assert_eq!(denied.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn unlock_sends_granted_callback_via_channel() {
    let svc = NlmService::new();
    let (tx, rx) = async_channel::unbounded::<Cookie>();

    svc.lock(
        None,
        make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0),
    )
    .await;

    let blocked = svc
        .lock(
            Some(tx),
            make_lock_args_with_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "bob", 200, 99),
        )
        .await;
    assert_eq!(blocked.stat, Nlm4Stats::Blocked);

    svc.unlock(make_unlock_args(FH_DEFAULT, "alice", 100, 1)).await;

    let received = rx.recv().await.expect("channel should not be closed");
    assert_eq!(received.raw(), 99);
}

#[tokio::test]
async fn unlock_no_callback_when_granted_tx_is_none() {
    let svc = NlmService::new();

    svc.lock(
        None,
        make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0),
    )
    .await;

    let blocked = svc
        .lock(
            None,
            make_lock_args_with_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "bob", 200, 99),
        )
        .await;
    assert_eq!(blocked.stat, Nlm4Stats::Blocked);

    svc.unlock(make_unlock_args(FH_DEFAULT, "alice", 100, 1)).await;

    let res = svc
        .lock(
            None,
            make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "charlie", 300, 0),
        )
        .await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}
