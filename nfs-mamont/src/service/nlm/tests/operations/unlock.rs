use crate::nlm::cookie::Cookie;
use crate::nlm::procedures::lock::Lock;
use crate::nlm::procedures::unlock::Unlock;
use crate::nlm::{Nlm4Stats, NlmCallbackReply};
use crate::service::nlm::tests::operations::DEFAULT_CRED;
use crate::service::nlm::tests::{
    create_empty_event_handler, make_lock_args_with_block, make_lock_args_without_block,
    make_unlock_args, FH_DEFAULT, LOCK_WHOLE_LENGTH,
};
use crate::service::nlm::NlmService;
use crate::task::global::nlm::nlm_event::NlmEventHandler;
use crate::task::{ProcCall, ProcMessage};

#[tokio::test]
async fn unlock_removes_lock_and_allows_new_lock() {
    let svc = NlmService::new();
    svc.lock(
        create_empty_event_handler(),
        make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0),
        &DEFAULT_CRED,
    )
    .await;
    svc.unlock(make_unlock_args(FH_DEFAULT, "alice", 100, 1), &DEFAULT_CRED).await;
    let res = svc
        .lock(
            create_empty_event_handler(),
            make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "bob", 200, 0),
            &DEFAULT_CRED,
        )
        .await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn unlock_on_nonexistent_lock_returns_granted() {
    let svc = NlmService::new();
    let res = svc.unlock(make_unlock_args(FH_DEFAULT, "nobody", 0, 0), &DEFAULT_CRED).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn unlock_preserves_cookie() {
    let svc = NlmService::new();
    let res = svc.unlock(make_unlock_args(FH_DEFAULT, "nobody", 0, 99), &DEFAULT_CRED).await;
    assert_eq!(res.cookie.raw(), 99);
}

#[tokio::test]
async fn unlock_auto_grants_pending_exclusive() {
    let svc = NlmService::new();
    // Alice holds [0, 100]
    svc.lock(
        create_empty_event_handler(),
        make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0),
        &DEFAULT_CRED,
    )
    .await;
    // Bob blocks on the same range
    let blocked = svc
        .lock(
            create_empty_event_handler(),
            make_lock_args_with_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "bob", 200, 0),
            &DEFAULT_CRED,
        )
        .await;
    assert_eq!(blocked.stat, Nlm4Stats::Blocked);
    // Alice unlocks -> Bob should be auto-granted
    svc.unlock(make_unlock_args(FH_DEFAULT, "alice", 100, 1), &DEFAULT_CRED).await;
    // Charlie should be denied because Bob now holds the lock
    let denied = svc
        .lock(
            create_empty_event_handler(),
            make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "charlie", 300, 0),
            &DEFAULT_CRED,
        )
        .await;
    assert_eq!(denied.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn unlock_sends_granted_callback_via_channel() {
    let svc = NlmService::new();
    let (tx, rx) = async_channel::unbounded::<ProcCall>();

    svc.lock(
        NlmEventHandler::new(tx.clone()),
        make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0),
        &DEFAULT_CRED,
    )
    .await;

    let blocked = svc
        .lock(
            NlmEventHandler::new(tx.clone()),
            make_lock_args_with_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "bob", 200, 99),
            &DEFAULT_CRED,
        )
        .await;
    assert_eq!(blocked.stat, Nlm4Stats::Blocked);

    svc.unlock(make_unlock_args(FH_DEFAULT, "alice", 100, 1), &DEFAULT_CRED).await;

    let received = rx.recv().await.expect("channel should not be closed");
    match received.proc_message {
        ProcMessage::Nlm4(nlm_call) => match nlm_call {
            NlmCallbackReply::Granted(test_args) => {
                assert_eq!(test_args.cookie.raw(), 99);
            }
        },
    }
}

#[tokio::test]
async fn unlock_no_callback_when_granted_tx_is_none() {
    let svc = NlmService::new();
    let (tx, rx) = async_channel::unbounded::<Cookie>();

    svc.lock(
        create_empty_event_handler(),
        make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0),
        &DEFAULT_CRED,
    )
    .await;
    let blocked = svc
        .lock(
            create_empty_event_handler(),
            make_lock_args_with_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "bob", 200, 99),
            &DEFAULT_CRED,
        )
        .await;
    assert_eq!(blocked.stat, Nlm4Stats::Blocked);

    svc.unlock(make_unlock_args(FH_DEFAULT, "alice", 100, 1), &DEFAULT_CRED).await;

    assert!(matches!(rx.try_recv(), Err(async_channel::TryRecvError::Empty)));
    drop(tx);
}
