use crate::nlm::lock::Nlm4Lock;
use crate::nlm::procedures::lock::Nlm4LockArgs;
use crate::vfs::file::Handle;

use super::{ActiveLock, LockRegistry};
use crate::consts::nfsv3::NFS3_FHSIZE;
use crate::nlm::cookie::Cookie;
use crate::nlm::OpaqueHandle;

mod operations;
mod ranges;
mod registry;

pub fn fill_fh(value: u8) -> [u8; NFS3_FHSIZE] {
    [value; NFS3_FHSIZE]
}

pub fn fill_opaque(value: u8) -> OpaqueHandle {
    OpaqueHandle::new([value; crate::consts::nlm::OPAQUE_HANDLE_SIZE])
}

pub fn make_active_lock(
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

pub fn push_lock(reg: &mut LockRegistry, fh_value: u8, exclusive: bool, offset: u64, length: u64) {
    reg.by_file
        .entry(fill_fh(fh_value))
        .or_default()
        .push(make_active_lock("a", 1, exclusive, offset, length, 1));
}

pub fn lock_args(
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

pub fn lock_args_block(
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
