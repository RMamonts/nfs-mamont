use nfs_mamont::nlm::cookie::Cookie;
use nfs_mamont::nlm::lock::Nlm4Lock;
use nfs_mamont::nlm::procedures::cancel::{Cancel, Nlm4CancelArgs};
use nfs_mamont::nlm::procedures::lock::Lock;
use nfs_mamont::nlm::{Name, Nlm4Stats};

use crate::nlm::tests::operations::DEFAULT_CRED;
use crate::nlm::tests::{
    fill_fh, fill_opaque, make_lock_args_with_block, make_lock_args_without_block, FH_DEFAULT,
    LOCK_WHOLE_LENGTH,
};
use crate::nlm::NlmService;

fn make_cancel_args(fh_value: u8, caller: &str, pid: i32, cookie_value: u64) -> Nlm4CancelArgs {
    Nlm4CancelArgs {
        cookie: Cookie::new(cookie_value),
        block: false,
        exclusive: true,
        lock: Nlm4Lock {
            caller_name: Name::new(caller.to_string()).unwrap(),
            file_handle: fill_fh(fh_value),
            opaque_handle: fill_opaque(1),
            system_identifier: pid,
            lock_offset: 0,
            lock_length: LOCK_WHOLE_LENGTH,
        },
    }
}

#[tokio::test]
async fn cancel_removes_blocked_request() {
    let svc = NlmService::new();
    svc.lock(
        make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0),
        &DEFAULT_CRED,
    )
    .await;
    let res = svc
        .lock(
            make_lock_args_with_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "bob", 200, 0),
            &DEFAULT_CRED,
        )
        .await;
    assert_eq!(res.stat, Nlm4Stats::Blocked);

    let res = svc.cancel(make_cancel_args(FH_DEFAULT, "bob", 200, 0), &DEFAULT_CRED).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);

    let res = svc.cancel(make_cancel_args(FH_DEFAULT, "bob", 200, 0), &DEFAULT_CRED).await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn cancel_on_nonexistent_returns_denied() {
    let svc = NlmService::new();
    let res = svc.cancel(make_cancel_args(FH_DEFAULT, "nobody", 0, 0), &DEFAULT_CRED).await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn cancel_preserves_cookie() {
    let svc = NlmService::new();
    svc.lock(
        make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0),
        &DEFAULT_CRED,
    )
    .await;
    svc.lock(
        make_lock_args_with_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "bob", 200, 0),
        &DEFAULT_CRED,
    )
    .await;
    let res = svc.cancel(make_cancel_args(FH_DEFAULT, "bob", 200, 55), &DEFAULT_CRED).await;
    assert_eq!(res.cookie.raw(), 55);
}

#[tokio::test]
async fn cancel_on_granted_lock_returns_granted() {
    let svc = NlmService::new();
    let res = svc
        .lock(
            make_lock_args_without_block(FH_DEFAULT, true, 0, 100, "alice", 100, 0),
            &DEFAULT_CRED,
        )
        .await;
    assert_eq!(res.stat, Nlm4Stats::Granted);

    let res = svc.cancel(make_cancel_args(FH_DEFAULT, "alice", 100, 0), &DEFAULT_CRED).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);
}
