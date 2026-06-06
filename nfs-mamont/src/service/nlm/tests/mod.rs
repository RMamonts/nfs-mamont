use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::lock::{Lock, Nlm4LockArgs};
use crate::nlm::procedures::unlock::{Nlm4UnlockArgs, Unlock};
use crate::nlm::Nlm4Stats;
use crate::vfs::file::Handle;

use super::{ActiveLock, LockRegistry, NlmService};
use crate::consts::nfsv3::NFS3_FHSIZE;
use crate::nlm::cookie::Cookie;
use crate::nlm::OpaqueHandle;

mod ranges;
mod registry;

pub(super) fn fill_fh(value: u8) -> [u8; NFS3_FHSIZE] {
    [value; NFS3_FHSIZE]
}

pub(super) fn fill_opaque(value: u8) -> OpaqueHandle {
    OpaqueHandle::new([value; crate::consts::nlm::OPAQUE_HANDLE_SIZE])
}

pub(super) fn make_active_lock(
    caller: &str,
    pid: i32,
    exclusive: bool,
    offset: u64,
    length: u64,
    opaque: u8,
) -> ActiveLock {
    ActiveLock::new(caller.into(), pid, exclusive, offset, length, fill_opaque(opaque))
        .expect("Test caller_name must be valid")
}

pub(super) fn push_lock(
    reg: &mut LockRegistry,
    fh_value: u8,
    exclusive: bool,
    offset: u64,
    length: u64,
) {
    reg.by_file
        .entry(fill_fh(fh_value))
        .or_default()
        .push(make_active_lock("a", 1, exclusive, offset, length, 1));
}

pub(super) fn lock_args(
    fh_value: u8,
    exclusive: bool,
    offset: u64,
    length: u64,
    caller: &str,
    pid: i32,
) -> Nlm4LockArgs {
    Nlm4LockArgs {
        cookie: Cookie::new(0),
        block: false,
        exclusive,
        lock: Nlm4Lock {
            caller_name: caller.into(),
            file_handle: Handle(fill_fh(fh_value)),
            opaque_handle: fill_opaque(1),
            system_identifier: pid,
            lock_offset: offset,
            lock_length: length,
        },
        reclaim: false,
        state: 0,
    }
}

pub(super) fn lock_args_block(
    fh_value: u8,
    exclusive: bool,
    offset: u64,
    length: u64,
    caller: &str,
    pid: i32,
) -> Nlm4LockArgs {
    Nlm4LockArgs {
        cookie: Cookie::new(0),
        block: true,
        exclusive,
        lock: Nlm4Lock {
            caller_name: caller.into(),
            file_handle: Handle(fill_fh(fh_value)),
            opaque_handle: fill_opaque(1),
            system_identifier: pid,
            lock_offset: offset,
            lock_length: length,
        },
        reclaim: false,
        state: 0,
    }
}

#[tokio::test]
async fn lock_unlock_lock_sequence_same_client() {
    let svc = NlmService::default();
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
    let svc = NlmService::default();
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
