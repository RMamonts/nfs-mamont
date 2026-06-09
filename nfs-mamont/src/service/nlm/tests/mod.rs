use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::lock::Nlm4LockArgs;
use crate::nlm::procedures::unlock::Nlm4UnlockArgs;
use crate::vfs::file::Handle;

use super::{ActiveLock, LockRegistry};
use crate::consts::nfsv3::NFS3_FHSIZE;
use crate::nlm::cookie::Cookie;
use crate::nlm::OpaqueHandle;

pub const FH_DEFAULT: u8 = 1;
pub const FH_OTHER: u8 = 2;
pub const LOCK_WHOLE_LENGTH: u64 = 100;

mod operations;
mod ranges;
mod registry;

pub fn fill_fh(value: u8) -> [u8; NFS3_FHSIZE] {
    [value; NFS3_FHSIZE]
}

pub fn fill_opaque(value: u8) -> OpaqueHandle {
    OpaqueHandle::new([value; crate::consts::nlm::OPAQUE_HANDLE_SIZE].to_vec()).unwrap()
}

pub fn make_active_lock(
    caller: &str,
    pid: i32,
    exclusive: bool,
    offset: u64,
    length: u64,
    opaque_value: u8,
) -> ActiveLock {
    ActiveLock::new(caller.into(), pid, exclusive, offset, length, fill_opaque(opaque_value))
        .expect("Test caller_name must be valid")
}

pub fn push_lock(reg: &mut LockRegistry, fh_value: u8, lock: ActiveLock) {
    reg.by_file.entry(fill_fh(fh_value)).or_default().push(lock);
}

pub fn make_lock_args_without_block(
    fh_value: u8,
    exclusive: bool,
    offset: u64,
    length: u64,
    caller: &str,
    pid: i32,
    cookie_value: u64,
) -> Nlm4LockArgs {
    Nlm4LockArgs {
        cookie: Cookie::new(cookie_value),
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

pub fn make_lock_args_with_block(
    fh_value: u8,
    exclusive: bool,
    offset: u64,
    length: u64,
    caller: &str,
    pid: i32,
    cookie_value: u64,
) -> Nlm4LockArgs {
    Nlm4LockArgs {
        cookie: Cookie::new(cookie_value),
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

fn make_unlock_args(fh_value: u8, caller: &str, pid: i32, cookie_value: u64) -> Nlm4UnlockArgs {
    Nlm4UnlockArgs {
        cookie: Cookie::new(cookie_value),
        lock: Nlm4Lock {
            caller_name: caller.into(),
            file_handle: Handle(fill_fh(fh_value)),
            opaque_handle: fill_opaque(2),
            system_identifier: pid,
            lock_offset: 0,
            lock_length: LOCK_WHOLE_LENGTH,
        },
    }
}
