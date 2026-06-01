use crate::nlm::procedures::test::{Nlm4TestArgs, Nlm4TestReply, Nlm4TestRes, Test};
use crate::nlm::Nlm4Stats;

use super::NlmService;

impl Test for NlmService {
    async fn test(&self, args: Nlm4TestArgs) -> Nlm4TestRes {
        let fh_bytes = args.lock.file_handle.0;
        let registry = self.locks.read().await;

        match registry.find_conflict(
            &fh_bytes,
            args.exclusive,
            args.lock.lock_offset,
            args.lock.lock_length,
        ) {
            Some(holder) => Nlm4TestRes {
                cookie: args.cookie,
                test_stat: Nlm4TestReply { stat: Nlm4Stats::Denied, holder: Some(holder) },
            },
            None => Nlm4TestRes {
                cookie: args.cookie,
                test_stat: Nlm4TestReply { stat: Nlm4Stats::Granted, holder: None },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nlm::cookie::Cookie;
    use crate::nlm::lock::Nlm4Lock;
    use crate::nlm::procedures::lock::Lock;
    use crate::nlm::Nlm4Stats;
    use crate::vfs::file::Handle;

    use super::super::{handle, lock_args, opaque};

    fn test_args(
        fh_byte: u8,
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
                file_handle: Handle(handle(fh_byte)),
                opaque_handle: opaque(2),
                system_identifier: 99,
                lock_offset: offset,
                lock_length: length,
            },
        }
    }

    #[tokio::test]
    async fn test_reports_granted_when_free() {
        let svc = NlmService::default();
        let res = svc.test(test_args(1, true, 0, 100, 0)).await;
        assert_eq!(res.test_stat.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn test_reports_denied_when_conflict() {
        let svc = NlmService::default();
        let svc_ref = &svc;
        svc_ref.lock(lock_args(1, true, 0, 100, "alice", 100)).await;
        let res = svc_ref.test(test_args(1, true, 0, 100, 0)).await;
        assert_eq!(res.test_stat.stat, Nlm4Stats::Denied);
    }

    #[tokio::test]
    async fn test_denied_holder_matches_conflicting_lock() {
        let svc = NlmService::default();
        let svc_ref = &svc;
        svc_ref.lock(lock_args(1, true, 0, 100, "alice", 42)).await;
        let res = svc_ref.test(test_args(1, true, 0, 100, 0)).await;
        let holder = res.test_stat.holder.expect("Denied response must have a holder");
        assert!(holder.exclusive);
        assert_eq!(holder.system_identifier, 42);
    }

    #[tokio::test]
    async fn test_no_holder_when_granted() {
        let svc = NlmService::default();
        let res = svc.test(test_args(1, true, 0, 100, 0)).await;
        assert_eq!(res.test_stat.stat, Nlm4Stats::Granted);
        assert!(res.test_stat.holder.is_none());
    }

    #[tokio::test]
    async fn test_preserves_cookie() {
        let svc = NlmService::default();
        let res = svc.test(test_args(1, true, 0, 100, 77)).await;
        assert_eq!(res.cookie.raw(), 77);
    }

    #[tokio::test]
    async fn test_reports_shared_compatible_as_granted() {
        let svc = NlmService::default();
        let svc_ref = &svc;
        svc_ref.lock(lock_args(1, false, 0, 100, "alice", 100)).await;
        let res = svc_ref.test(test_args(1, false, 10, 20, 0)).await;
        assert_eq!(res.test_stat.stat, Nlm4Stats::Granted);
    }
}
