//! Defines NFSv4 operation types.
//!
//! Each operation (RFC 7530 §16) is described in its own submodule, which
//! provides the argument and result types for that operation. Types shared
//! across several operations are placed in the `file` module.

use num_derive::{FromPrimitive, ToPrimitive};

pub mod access;
pub mod file;

/// NFSv4 success status code.
pub const STATUS_OK: u32 = 0;

/// NFSv4 status codes (`nfsstat4`), RFC 7530 §13.1.
///
/// These are operation result statuses. `0` is success (NFS4_OK); all other
/// values are errors.
#[derive(Debug, Copy, Clone, PartialEq, Eq, ToPrimitive, FromPrimitive)]
#[repr(u32)]
pub enum Error {
    /// NFS4_OK - successful execution of the operation.
    Ok = 0,
    /// NFS4ERR_PERM - not owner.
    Permission = 1,
    /// NFS4ERR_NOENT - no such file or directory.
    NoEntry = 2,
    /// NFS4ERR_IO - I/O error.
    IO = 5,
    /// NFS4ERR_NXIO - no such device or address.
    Nxio = 6,
    /// NFS4ERR_ACCESS - permission denied.
    Access = 13,
    /// NFS4ERR_EXIST - file exists.
    Exist = 17,
    /// NFS4ERR_XDEV - attempt to do a cross-device hard link.
    XDev = 18,
    /// NFS4ERR_NOTDIR - not a directory.
    NotDir = 20,
    /// NFS4ERR_ISDIR - is a directory.
    IsDir = 21,
    /// NFS4ERR_INVAL - invalid argument.
    InvalidArgument = 22,
    /// NFS4ERR_FBIG - file too large.
    FileTooLarge = 27,
    /// NFS4ERR_NOSPC - no space left on device.
    NoSpace = 28,
    /// NFS4ERR_ROFS - read-only file system.
    ReadOnlyFs = 30,
    /// NFS4ERR_MLINK - too many hard links.
    TooManyLinks = 31,
    /// NFS4ERR_NAMETOOLONG - filename in an operation was too long.
    NameTooLong = 63,
    /// NFS4ERR_NOTEMPTY - directory not empty.
    NotEmpty = 66,
    /// NFS4ERR_DQUOT - quota exceeded.
    QuotaExceeded = 69,
    /// NFS4ERR_STALE - invalid file handle.
    StaleFile = 70,
    /// NFS4ERR_BADHANDLE - illegal NFS file handle.
    BadFileHandle = 10001,
    /// NFS4ERR_BAD_COOKIE - readdir cookie is stale.
    BadCookie = 10003,
    /// NFS4ERR_NOTSUPP - operation is not supported.
    NotSupported = 10004,
    /// NFS4ERR_TOOSMALL - buffer or request is too small.
    TooSmall = 10005,
    /// NFS4ERR_SERVERFAULT - an error occurred on the server.
    ServerFault = 10006,
    /// NFS4ERR_BADTYPE - an attempt was made to create an object of a type
    /// not supported by the server.
    BadType = 10007,
    /// NFS4ERR_DELAY - the server was not able to complete the request in a
    /// timely fashion.
    Delay = 10008,
    /// NFS4ERR_SAME - the reserve block attribute value is unchanged.
    Same = 10009,
    /// NFS4ERR_DENIED - the lock request was denied.
    Denied = 10010,
    /// NFS4ERR_EXPIRED - the lease has expired.
    Expired = 10011,
    /// NFS4ERR_LOCKED - the file or directory is locked.
    Locked = 10012,
    /// NFS4ERR_GRACE - the operation was attempted during a grace period.
    Grace = 10013,
    /// NFS4ERR_FHEXPIRED - the file handle has expired.
    FhExpired = 10014,
    /// NFS4ERR_SHARE_DENIED - the request to share a file was denied.
    ShareDenied = 10015,
    /// NFS4ERR_WRONGSEC - the security mechanism used is wrong.
    WrongSec = 10016,
    /// NFS4ERR_CLID_INUSE - the client id is already in use.
    ClientIdInUse = 10017,
    /// NFS4ERR_RESOURCE - resource exhaustion.
    Resource = 10018,
    /// NFS4ERR_MOVED - the file was moved.
    Moved = 10019,
    /// NFS4ERR_NOFILEHANDLE - there is no file handle for the operation.
    NoFileHandle = 10020,
    /// NFS4ERR_MINOR_VERS_MISMATCH - the minor version is not supported.
    MinorVersMismatch = 10021,
    /// NFS4ERR_STALE_CLIENTID - the client id is stale.
    StaleClientId = 10022,
    /// NFS4ERR_STALE_STATEID - the state id is stale.
    StaleStateId = 10023,
    /// NFS4ERR_OLD_STATEID - the state id is old.
    OldStateId = 10024,
    /// NFS4ERR_BAD_STATEID - the state id is invalid.
    BadStateId = 10025,
    /// NFS4ERR_BAD_SEQID - the sequence id is invalid.
    BadSeqId = 10026,
    /// NFS4ERR_NOT_SAME - the verify operation found different attributes.
    NotSame = 10027,
    /// NFS4ERR_LOCK_RANGE - the lock range is invalid.
    LockRange = 10028,
    /// NFS4ERR_SYMLINK - the file is a symbolic link.
    Symlink = 10029,
    /// NFS4ERR_RESTOREFH - restorefh failed.
    RestoreFh = 10030,
    /// NFS4ERR_LEASE_MOVED - the lease was moved.
    LeaseMoved = 10031,
    /// NFS4ERR_ATTRNOTSUPP - the attribute is not supported.
    AttrNotSupp = 10032,
    /// NFS4ERR_NO_GRACE - the operation was attempted outside of the grace
    /// period.
    NoGrace = 10033,
    /// NFS4ERR_RECLAIM_BAD - the reclaim operation is invalid.
    ReclaimBad = 10034,
    /// NFS4ERR_RECLAIM_CONFLICT - the reclaim operation conflicts.
    ReclaimConflict = 10035,
    /// NFS4ERR_BADXDR - the XDR encoding of the request is invalid.
    BadXdr = 10036,
    /// NFS4ERR_LOCKS_HELD - the file has locks held.
    LocksHeld = 10037,
    /// NFS4ERR_OPENMODE - the operation is not allowed for the open mode.
    OpenMode = 10038,
    /// NFS4ERR_BADOWNER - the owner attribute value is invalid.
    BadOwner = 10039,
    /// NFS4ERR_BADCHAR - the character is invalid.
    BadChar = 10040,
    /// NFS4ERR_BADNAME - the name is invalid.
    BadName = 10041,
    /// NFS4ERR_BAD_RANGE - the lock range is invalid.
    BadRange = 10042,
    /// NFS4ERR_LOCK_NOTSUPP - the lock operation is not supported.
    LockNotSupp = 10043,
    /// NFS4ERR_OP_ILLEGAL - the operation is illegal.
    OpIllegal = 10044,
    /// NFS4ERR_DEADLOCK - the lock operation would deadlock.
    Deadlock = 10045,
    /// NFS4ERR_FILE_OPEN - the file is open.
    FileOpen = 10046,
    /// NFS4ERR_ADMIN_REVOKED - the state was revoked by the administrator.
    AdminRevoked = 10047,
    /// NFS4ERR_CB_PATH_DOWN - the callback path is down.
    CallbackPathDown = 10048,
}
