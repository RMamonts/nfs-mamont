use crate::nlm::procedures::cancel::{Cancel, Nlm4CancelArgs, Nlm4CancelRes};
use crate::nlm::Nlm4Stats;

use super::NlmService;

impl Cancel for NlmService {
    async fn cancel(&self, args: Nlm4CancelArgs) -> Nlm4CancelRes {
        let fh_bytes = args.lock.file_handle.0;
        let caller_name = args.lock.caller_name;
        let system_identifier = args.lock.system_identifier;

        let mut registry = self.locks.write().await;
        registry.remove_by_owner(&fh_bytes, &caller_name, system_identifier);

        Nlm4CancelRes { cookie: args.cookie, stat: Nlm4Stats::Granted }
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
    use crate::nlm::procedures::lock::{Lock, Nlm4LockArgs};
    use crate::nlm::Nlm4Stats;
    use crate::vfs::file::Handle;

    fn handle(byte: u8) -> [u8; NFS3_FHSIZE] {
        [byte; NFS3_FHSIZE]
    }

    fn opaque(val: u8) -> OpaqueHandle {
        OpaqueHandle::new([val; OPAQUE_HANDLE_SIZE])
    }

    fn lock_args(fh_byte: u8, exclusive: bool, offset: u64, length: u64, caller: &str, pid: i32) -> Nlm4LockArgs {
        Nlm4LockArgs {
            cookie: Cookie::new(0),
            block: false,
            exclusive,
            lock: Nlm4Lock {
                caller_name: caller.into(),
                file_handle: Handle(handle(fh_byte)),
                opaque_handle: opaque(1),
                system_identifier: pid,
                lock_offset: offset,
                lock_length: length,
            },
            reclaim: false,
            state: 0,
        }
    }

    fn cancel_args(fh_byte: u8, caller: &str, pid: i32, cookie_val: u64) -> Nlm4CancelArgs {
        Nlm4CancelArgs {
            cookie: Cookie::new(cookie_val),
            block: false,
            exclusive: true,
            lock: Nlm4Lock {
                caller_name: caller.into(),
                file_handle: Handle(handle(fh_byte)),
                opaque_handle: opaque(2),
                system_identifier: pid,
                lock_offset: 0,
                lock_length: 100,
            },
        }
    }

    #[tokio::test]
    async fn cancel_removes_lock_and_allows_new_lock() {
        let svc = super::NlmService::default();
        let svc_ref = &svc;
        svc_ref.lock(lock_args(1, true, 0, 100, "alice", 100)).await;
        svc_ref.cancel(cancel_args(1, "alice", 100, 0)).await;
        let res = svc_ref.lock(lock_args(1, true, 0, 100, "bob", 200)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn cancel_on_nonexistent_lock_returns_granted() {
        let svc = super::NlmService::default();
        let res = svc.cancel(cancel_args(1, "nobody", 0, 0)).await;
        assert_eq!(res.stat, Nlm4Stats::Granted);
    }

    #[tokio::test]
    async fn cancel_preserves_cookie() {
        let svc = super::NlmService::default();
        let res = svc.cancel(cancel_args(1, "nobody", 0, 55)).await;
        assert_eq!(res.cookie.raw(), 55);
    }
}
