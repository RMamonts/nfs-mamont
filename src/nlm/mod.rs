use async_trait::async_trait;

// TODO: naming
use crate::vfs::file;

#[allow(dead_code)]
pub const NETOBJ_SIZE: usize = 8;

#[allow(dead_code)]
pub const MAX_CLIENT_NAME_LEN: usize = 255;

// TODO: naming
#[allow(dead_code)]
pub enum Response {
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
    DeniedGracePeriod = 4,
    /// The request could not be granted and blocking would cause a deadlock.
    Deadlock = 5,
    /// The call failed because the remote file system is read-only.
    ReadonlyFileSystem = 6,
    /// The call failed because it uses an invalid file handle.
    InvalidFileHandler = 7,
    /// The call failed because it specified a length or offset
    /// that exceeds the range supported by the server.
    ExceededRange = 8,
    /// The call failed for some reason not already listed.
    /// The client should probably retry the request.
    Failed = 9,
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

/// Information needed to request a lock on a serverinformation needed to request a lock on a server.
#[allow(dead_code)]
pub struct LockArgs {
    pub cookie: Netobj,
    pub block: bool,
    pub exclusive: bool,
    pub actual_lock: LockRequest,
    pub reclaim: bool,
    pub state: u32
}

/// The result of the lock requests,
/// returned by all of the main lock routines except for NLM_TEST.
#[allow(dead_code)]
pub struct LockResult {
    /// Host that is making the request.
    pub cookie: Netobj,
    /// Actual response to the lock request.
    pub stat: Response
}

/// Information needed to cancel an outstanding lock request.
/// The data in the CancelArgs structure must exactly match
/// the corresponding information in the LockArgs structure
/// of the outstanding lock request to be cancelled.
#[allow(dead_code)]
pub struct CancelArgs {
    pub cookie: Netobj,
    pub block: bool,
    pub exclusive: bool,
    pub actual_lock: LockRequest,
}

/// Information needed to remove a previously established lock.
#[allow(dead_code)]
pub struct UnlockArgs {
    pub cookie: Netobj,
    pub actual_lock: LockRequest,
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
    pub cookie: Netobj,
    /// Actual response to the share request.
    pub stats: Response,
    /// Sequence number.
    pub sequence: i32,
}

/// Results of an NLM_SHARE or NLM_UNSHARE procedure call
#[allow(dead_code)]
pub struct Notify {
    // TODO: String ???
    pub name: String,
    // TODO: type ?
    pub state: i64,
}

/// Arguments for the TEST procedure.
#[allow(dead_code)]
pub struct TestArgs {
    pub cookie: Netobj,
    pub exclusive: bool,
    pub actual_lock: LockRequest,
}

#[allow(dead_code)]
pub struct TestResult {
    pub cookie: Netobj,
    pub test_stats: Option<Holder>
}

#[async_trait]
pub trait Nlm: Sync + Send {
    async fn null(&self, promise: impl promise::Null);

    async fn test(&self, args: TestArgs, promise: impl promise::Test);

    async fn lock(&self, args: LockArgs, promise: impl promise::Lock);

    async fn cancel(&self, args: CancelArgs, promise: impl promise::Cancel);

    async fn unlock(&self, args: UnlockArgs, promise: impl promise::Unlock);

    // Server-to-Client callback
    async fn granted(&self, args: TestArgs, promise: impl promise::Granted);

    // --- Message passing (Asynchronous) procedures ---
    // These generally return void.

    async fn test_msg(&self, args: TestArgs, promise: impl promise::Void);

    async fn lock_msg(&self, args: LockArgs, promise: impl promise::Void);

    async fn cancel_msg(&self, args: CancelArgs, promise: impl promise::Void);

    async fn unlock_msg(&self, args: UnlockArgs, promise: impl promise::Void);

    async fn granted_msg(&self, args: TestArgs, promise: impl promise::Void);

    // --- Message Responses (Callback results) ---
    // These are sent by the server back to client (or vice versa) to report results of _msg calls.

    async fn test_res(&self, res: TestResult, promise: impl promise::MsgResult);

    async fn lock_res(&self, res: LockResult, promise: impl promise::MsgResult);

    async fn cancel_res(&self, res: LockResult, promise: impl promise::MsgResult);

    async fn unlock_res(&self, res: LockResult, promise: impl promise::MsgResult);

    async fn granted_res(&self, res: LockResult, promise: impl promise::MsgResult);

    // --- DOS Sharing ---

    async fn share(&self, args: ShareArgs, promise: impl promise::Share);

    async fn unshare(&self, args: ShareArgs, promise: impl promise::Share);

    async fn nm_lock(&self, args: LockArgs, promise: impl promise::Lock);

    async fn free_all(&self, args: Notify, promise: impl promise::Void);
}

mod promise {
    use crate::nlm::{TestResult, LockResult, ShareResult};

    pub trait Null {
        fn keep(self);
    }

    pub trait Test {
        fn keep(self, result: TestResult);
    }

    pub trait Lock {
        fn keep(self, result: LockResult);
    }

    pub trait Cancel {
        fn keep(self, result: LockResult);
    }

    pub trait Unlock {
        fn keep(self, result: LockResult);
    }

    pub trait Granted {
        fn keep(self, result: LockResult);
    }

    pub trait Share {
        fn keep(self, result: ShareResult);
    }

    pub trait MsgResult {
        fn keep(self);
    }

    /// Generic promise for procedures that return void (or just success/fail without data).
    pub trait Void {
        fn keep(self);
    }
}
