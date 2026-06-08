use super::super::{
    fill_fh, fill_opaque, make_lock_args_without_block, FH_DEFAULT, LOCK_WHOLE_LENGTH,
};
use crate::nlm::cookie::Cookie;
use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::lock::Lock;
use crate::nlm::procedures::test::{Nlm4TestArgs, Test};
use crate::nlm::Nlm4Stats;
use crate::service::nlm::NlmService;
use crate::vfs::file::Handle;

fn make_test_args(
    fh_value: u8,
    exclusive: bool,
    offset: u64,
    length: u64,
    cookie_val: u64,
) -> Nlm4TestArgs {
    Nlm4TestArgs {
        cookie: Cookie::new(cookie_val),
        exclusive,
        lock: Nlm4Lock {
            caller_name: "tester".into(),
            file_handle: Handle(fill_fh(fh_value)),
            opaque_handle: fill_opaque(2),
            system_identifier: 99,
            lock_offset: offset,
            lock_length: length,
        },
    }
}

#[tokio::test]
async fn test_reports_granted_when_free() {
    let svc = NlmService::new();
    let res = svc.test(make_test_args(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, 0)).await;
    assert_eq!(res.test_stat.stat, Nlm4Stats::Granted);
}

#[tokio::test]
async fn test_reports_denied_when_conflict() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 100, 0))
        .await;
    let res = svc.test(make_test_args(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, 0)).await;
    assert_eq!(res.test_stat.stat, Nlm4Stats::Denied);
}

#[tokio::test]
async fn test_denied_holder_matches_conflicting_lock() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, "alice", 42, 0))
        .await;
    let res = svc.test(make_test_args(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, 0)).await;
    let holder = res.test_stat.holder.expect("Denied response must have a holder");
    assert!(holder.exclusive);
    assert_eq!(holder.system_identifier, 42);
}

#[tokio::test]
async fn test_no_holder_when_granted() {
    let svc = NlmService::new();
    let res = svc.test(make_test_args(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, 0)).await;
    assert_eq!(res.test_stat.stat, Nlm4Stats::Granted);
    assert!(res.test_stat.holder.is_none());
}

#[tokio::test]
async fn test_preserves_cookie() {
    let svc = NlmService::new();
    let res = svc.test(make_test_args(FH_DEFAULT, true, 0, LOCK_WHOLE_LENGTH, 77)).await;
    assert_eq!(res.cookie.raw(), 77);
}

#[tokio::test]
async fn test_reports_shared_compatible_as_granted() {
    let svc = NlmService::new();
    svc.lock(make_lock_args_without_block(FH_DEFAULT, false, 0, 100, "alice", 100, 0)).await;
    let res = svc.test(make_test_args(FH_DEFAULT, false, 10, 20, 0)).await;
    assert_eq!(res.test_stat.stat, Nlm4Stats::Granted);
}
