use crate::nlm::procedures::lock::{Lock, Nlm4LockArgs, Nlm4LockRes};
use crate::nlm::Nlm4Stats;

use super::{ActiveLock, NlmService};

impl Lock for NlmService {
    async fn lock(&self, args: Nlm4LockArgs) -> Nlm4LockRes {
        let fh_bytes = args.lock.file_handle.0;
        let exclusive = args.exclusive;
        let offset = args.lock.lock_offset;
        let length = args.lock.lock_length;
        let caller_name = args.lock.caller_name.clone();
        let system_identifier = args.lock.system_identifier;
        let opaque_handle = args.lock.opaque_handle.clone();

        let mut registry = self.locks.write().await;

        if registry.find_conflict(&fh_bytes, exclusive, offset, length).is_some() {
            return Nlm4LockRes { cookie: args.cookie, stat: Nlm4Stats::Denied };
        }

        registry.by_file.entry(fh_bytes).or_default().push(ActiveLock {
            caller_name,
            system_identifier,
            exclusive,
            offset,
            length,
            opaque_handle,
        });

        Nlm4LockRes { cookie: args.cookie, stat: Nlm4Stats::Granted }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::nfsv3::NFS3_FHSIZE;
    use crate::consts::nlm::OPAQUE_HANDLE_SIZE;
    use crate::nlm::cookie::Cookie;
    use crate::nlm::lock::Nlm4Lock;
    use crate::nlm::OpaqueHandle;
    use crate::vfs::file::Handle;

    fn handle(byte: u8) -> [u8; NFS3_FHSIZE] {
        [byte; NFS3_FHSIZE]
    }

    fn opaque(val: u8) -> OpaqueHandle {
        OpaqueHandle::new([val; OPAQUE_HANDLE_SIZE])
    }

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
                file_handle: Handle(handle(fh_byte)),
                opaque_handle: opaque(1),
                system_identifier: 42,
                lock_offset: offset,
                lock_length: length,
            },
            reclaim: false,
            state: 0,
        }
    }

    #[tokio::test]
    async fn lock_grants_exclusive_lock() {
        let svc = NlmService::default();
        let res = svc.lock(lock_args(1, true, 0, 100, 0)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn lock_denies_conflicting_exclusive() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, true, 0, 100, 0)).await;
        let res = svc.lock(lock_args(1, true, 0, 100, 1)).await;
        assert_eq!(res.stat, Nlm4Stats::Denied);
    }

    #[tokio::test]
    async fn lock_allows_shared_overlapping() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, false, 0, 100, 0)).await;
        let res = svc.lock(lock_args(1, false, 10, 20, 1)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn lock_denies_shared_against_exclusive() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, true, 0, 100, 0)).await;
        let res = svc.lock(lock_args(1, false, 10, 20, 1)).await;
        assert_eq!(res.stat, Nlm4Stats::Denied);
    }

    #[tokio::test]
    async fn lock_denies_exclusive_against_shared() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, false, 0, 100, 0)).await;
        let res = svc.lock(lock_args(1, true, 10, 20, 1)).await;
        assert_eq!(res.stat, Nlm4Stats::Denied);
    }

    #[tokio::test]
    async fn lock_allows_different_files() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, true, 0, 100, 0)).await;
        let res = svc.lock(lock_args(2, true, 0, 100, 1)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn lock_allows_non_overlapping_ranges() {
        let svc = NlmService::default();
        svc.lock(lock_args(1, true, 0, 50, 0)).await;
        let res = svc.lock(lock_args(1, true, 50, 50, 1)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn lock_preserves_cookie() {
        let svc = NlmService::default();
        let res = svc.lock(lock_args(1, true, 0, 100, 42)).await;
        assert_eq!(res.cookie.raw(), 42);
    }
}
