use super::super::{fill_fh, fill_opaque, lock_args, lock_args_block};
use crate::nlm::cookie::Cookie;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::cancel::{Cancel, Nlm4CancelArgs};
use crate::nlm::procedures::lock::Lock;
use crate::nlm::Nlm4Stats;
use crate::service::nlm::NlmService;
use crate::vfs::file::Handle;

fn make_exclusive_non_blocking_cancel_args_of_len_100(
    fh_byte: u8,
    caller: &str,
    pid: i32,
    cookie_value: u64,
) -> Nlm4CancelArgs {
    Nlm4CancelArgs {
        cookie: Cookie::new(cookie_value),
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
    let svc = NlmService::new();
    svc.lock(lock_args(1, true, 0, 100, "alice", 100)).await;
    let res = svc.lock(lock_args_block(1, true, 0, 100, "bob", 200)).await;
    assert_eq!(res.stat, Nlm4Stats::Blocked);

    let res =
        svc.cancel(make_exclusive_non_blocking_cancel_args_of_len_100(1, "bob", 200, 0)).await;
    assert_eq!(res.stat, Nlm4Stats::Granted);

    let res =
        svc.cancel(make_exclusive_non_blocking_cancel_args_of_len_100(1, "bob", 200, 0)).await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn cancel_on_nonexistent_returns_denied() {
    let svc = NlmService::new();
    let res =
        svc.cancel(make_exclusive_non_blocking_cancel_args_of_len_100(1, "nobody", 0, 0)).await;
    assert_eq!(res.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn cancel_preserves_cookie() {
    let svc = NlmService::new();
    svc.lock(lock_args(1, true, 0, 100, "alice", 100)).await;
    svc.lock(lock_args_block(1, true, 0, 100, "bob", 200)).await;
    let res =
        svc.cancel(make_exclusive_non_blocking_cancel_args_of_len_100(1, "bob", 200, 55)).await;
    assert_eq!(res.cookie.raw(), 55);
}
