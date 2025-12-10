// TODO: naming
use crate::vfs::file;

#[allow(dead_code)]
pub const NETOBJ_SIZE: usize = 8;

#[allow(dead_code)]
pub const MAX_CLIENT_NAME_LEN: usize = 255;

// TODO: naming
#[allow(dead_code)]
pub enum Stat {
    /// Indicates that the procedure call completed successfully.
    Granted = 0,
    /// Indicates that the request failed.
    Denied = 1,
    /// Indicates that the procedure call failed
    /// because the server NLM could not allocate
    /// the resources needed to process the request.
    DeniedNoLocks = 2,
    /// Indicates the blocking request cannot be granted immediately.
    /// The server NLM will make a call-back to the client
    /// with an NLM_GRANTED procedure call when the lock can be granted.
    Blocked = 3,
    /// Indicates that the procedure call failed
    /// because the server has recently been rebooted
    /// and the server NLM is re-establishing existing locks,
    /// and is not yet ready to accept normal service requests.
    DeniedGracePeriod = 4
}

#[allow(dead_code)]
pub struct ClientName(pub [u8; MAX_CLIENT_NAME_LEN]);

#[allow(dead_code)]
pub struct Netobj(pub [u8; NETOBJ_SIZE]);

/// Holder of a lock.
#[allow(dead_code)]
pub struct Holder {
    /// Tells whether the holder has an exclusive lock or a shared lock
    pub exclusive: bool,
    /// Identifies the process that is holding the lock.
    pub pid: u32,
    /// An opaque object that identifies the host,
    /// or a process on the host, that is holding the lock.
    pub owner: Netobj,
    /// Identifies the offset of the region that is locked.
    pub locked_offset: u64,
    /// Identifies the length of the region that is locked.
    pub locked_len: u64,
}

/// Lock request.
#[allow(dead_code)]
pub struct LockRequest {
    /// Host that is making the request.
    pub client_name: ClientName,
    /// File to lock. NFS Server id, opaque to client.
    pub nfs_fh: file::Handle, 
    /// An opaque object that identifies the host,
    /// or a process on the host, that is making the request.
    pub owner: Netobj,
    /// Process that is making the request.
    pub pid: u32,
    /// Offset of the region that is locked.
    pub locked_offset: u64,
    /// Length of the region that is locked.
    /// A l_len of zero means "to end-of-file."
    pub locked_len: u64,
}

/// The result of the lock requests,
/// returned by all of the main lock routines except for NLM_TEST.
#[allow(dead_code)]
pub struct LockResult {
    /// Host that is making the request.
    pub cookie: Netobj,
    /// Actual response to the lock request.
    pub stat: Stats
}

// TODO: naming
#[allow(dead_code)]
pub enum ShareMode {
    /// Deny none.
    DenyNone = 0,
    /// Deny read.
    DenyRead = 1,
    /// Deny write.
    DenyWrite = 2,
    /// Deny read/write.
    DenyReadWrite = 3,
}

// TODO: naming
#[allow(dead_code)]
pub enum AccessMode {
    /// None.
    None = 0,
    /// Read-only.
    Read = 1,
    /// Write-only.
    Write = 2,
    /// Read/Write.
    ReadWrite = 3,
}

/// Lock request.
#[allow(dead_code)]
pub struct LockRequest {
    /// Host that is making the request.
    pub client_name: ClientName,
    /// File to lock. NFS Server id, opaque to client.
    pub nfs_fh: file::Handle, 
    /// An opaque object that identifies the host,
    /// or a process on the host, that is making the request.
    pub owner: Netobj,
    /// Process that is making the request.
    pub pid: u32,
    /// Offset of the region that is locked.
    pub locked_offset: u64,
    /// Length of the region that is locked.
    /// A l_len of zero means "to end-of-file."
    pub locked_len: u64,
}

/// DOS file sharing description.
#[allow(dead_code)]
pub struct Share {
    /// Host that is making the request.
    pub client_name: ClientName,
    /// File to be operated on. NFS Server id, opaque to client.
    pub nfs_fh: file::Handle, 
    /// An opaque object that identifies the host,
    /// or a process on the host, that is making the request.
    pub owner: Netobj,
    /// File-sharing mode. Identifies what is allowed to other clients.
    pub share_mod: ShareMode,
    /// Access mode, requested by the client.
    pub access_mode: AccessMode,
}

/// Information needed to uniquely specify a share operation.
/// Arguments for an NLM_SHARE or NLM_UNSHARE
#[allow(dead_code)]
pub struct ShareArgs {
    pub cookie: Netobj,
    /// Actual share data.
    pub share: Share,
    /// Must be true if the client is attempting to
    /// reclaim a previously-granted sharing request.
    pub reclaim: bool,

}

/// Results of an NLM_SHARE or NLM_UNSHARE procedure call
#[allow(dead_code)]
pub struct ShareResult {
    /// Actual response to the share request.
    pub stat: Stats,
}

/// Results of an NLM_SHARE or NLM_UNSHARE procedure call
#[allow(dead_code)]
pub struct Notify {
    /// Actual response to the share request.
    pub stat: Stats,
}



// version NLM4_VERS {
//     void NLMPROC4_NULL(void) = 0;
//     nlm4_testres NLMPROC4_TEST(nlm4_testargs) = 1;
//     nlm4_res NLMPROC4_LOCK(nlm4_lockargs) = 2;
//     nlm4_res NLMPROC4_CANCEL(nlm4_cancargs) = 3;
//     nlm4_res NLMPROC4_UNLOCK(nlm4_unlockargs) = 4;
//     nlm4_res NLMPROC4_GRANTED(nlm4_testargs) = 5;
//     void NLMPROC4_TEST_MSG(nlm4_testargs) = 6;
//     void NLMPROC4_LOCK_MSG(nlm4_lockargs) = 7;
//     void NLMPROC4_CANCEL_MSG(nlm4_cancargs) = 8;
//     void NLMPROC4_UNLOCK_MSG(nlm4_unlockargs) = 9;
//     void NLMPROC4_GRANTED_MSG(nlm4_testargs) = 10;
//     void NLMPROC4_TEST_RES(nlm4_testres) = 11;
//     void NLMPROC4_LOCK_RES(nlm4_res) = 12;
//     void NLMPROC4_CANCEL_RES(nlm4_res) = 13;
//     void NLMPROC4_UNLOCK_RES(nlm4_res) = 14;
//     void NLMPROC4_GRANTED_RES(nlm4_res) = 15;
//     nlm4_shareres NLMPROC4_SHARE(nlm4_shareargs) = 20;
//     nlm4_shareres NLMPROC4_UNSHARE(nlm4_shareargs) = 21;
//     nlm4_res NLMPROC4_NM_LOCK(nlm4_lockargs) = 22;
//     void NLMPROC4_FREE_ALL(nlm4_notify) = 23;
// } = 4;