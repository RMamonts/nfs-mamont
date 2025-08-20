//! NFS version 4 XDR definitions
//!
//! This module contains all XDR type definitions for NFSv4 protocol
//! as specified in RFC 7530.
#![allow(dead_code)]
#![allow(unused_variables)]
pub mod operations;

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use num_derive::{FromPrimitive, ToPrimitive};
pub use operations::{COMPOUND4args, COMPOUND4res, NULL4args, NULL4res};

use crate::protocol::nfs::v4::NFSv4FS;
use crate::xdr;

#[allow(non_camel_case_types)]
pub type seqid4 = u32;
#[allow(non_camel_case_types)]
pub type clientid4 = u64;
/// NFS version 4 status codes as defined in RFC 7530
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Default, FromPrimitive, ToPrimitive, PartialEq, Eq)]
pub enum nfsstat4 {
    #[default]
    NFS4_OK = 0,
    NFS4ERR_PERM = 1,
    NFS4ERR_NOENT = 2,
    NFS4ERR_IO = 5,
    NFS4ERR_NXIO = 6,
    NFS4ERR_ACCESS = 13,
    NFS4ERR_EXIST = 17,
    NFS4ERR_XDEV = 18,
    NFS4ERR_NOTDIR = 20,
    NFS4ERR_ISDIR = 21,
    NFS4ERR_INVAL = 22,
    NFS4ERR_FBIG = 27,
    NFS4ERR_NOSPC = 28,
    NFS4ERR_ROFS = 30,
    NFS4ERR_MLINK = 31,
    NFS4ERR_NAMETOOLONG = 63,
    NFS4ERR_NOTEMPTY = 66,
    NFS4ERR_DQUOT = 69,
    NFS4ERR_STALE = 70,
    NFS4ERR_BADHANDLE = 10001,
    NFS4ERR_BAD_COOKIE = 10003,
    NFS4ERR_NOTSUPP = 10004,
    NFS4ERR_TOOSMALL = 10005,
    NFS4ERR_SERVERFAULT = 10006,
    NFS4ERR_BADTYPE = 10007,
    NFS4ERR_DELAY = 10008,
    NFS4ERR_SAME = 10009,
    NFS4ERR_DENIED = 10010,
    NFS4ERR_EXPIRED = 10011,
    NFS4ERR_LOCKED = 10012,
    NFS4ERR_GRACE = 10013,
    NFS4ERR_FHEXPIRED = 10014,
    NFS4ERR_SHARE_DENIED = 10015,
    NFS4ERR_WRONGSEC = 10016,
    NFS4ERR_CLID_INUSE = 10017,
    NFS4ERR_RESOURCE = 10018,
    NFS4ERR_MOVED = 10019,
    NFS4ERR_NOFILEHANDLE = 10020,
    NFS4ERR_MINOR_VERS_MISMATCH = 10021,
    NFS4ERR_STALE_CLIENTID = 10022,
    NFS4ERR_STALE_STATEID = 10023,
    NFS4ERR_OLD_STATEID = 10024,
    NFS4ERR_BAD_STATEID = 10025,
    NFS4ERR_BAD_SEQID = 10026,
    NFS4ERR_NOT_SAME = 10027,
    NFS4ERR_LOCK_RANGE = 10028,
    NFS4ERR_SYMLINK = 10029,
    NFS4ERR_RESTOREFH = 10030,
    NFS4ERR_LEASE_MOVED = 10031,
    NFS4ERR_ATTRNOTSUPP = 10032,
    NFS4ERR_NO_GRACE = 10033,
    NFS4ERR_RECLAIM_BAD = 10034,
    NFS4ERR_RECLAIM_CONFLICT = 10035,
    NFS4ERR_BADXDR = 10036,
    NFS4ERR_LOCKS_HELD = 10037,
    NFS4ERR_OPENMODE = 10038,
    NFS4ERR_BADOWNER = 10039,
    NFS4ERR_BADCHAR = 10040,
    NFS4ERR_BADNAME = 10041,
    NFS4ERR_BAD_RANGE = 10042,
    NFS4ERR_LOCK_NOTSUPP = 10043,
    NFS4ERR_OP_ILLEGAL = 10044,
    NFS4ERR_DEADLOCK = 10045,
    NFS4ERR_FILE_OPEN = 10046,
    NFS4ERR_ADMIN_REVOKED = 10047,
    NFS4ERR_CB_PATH_DOWN = 10048,
    NFS4ERR_BADIOMODE = 10049,
    NFS4ERR_BADLAYOUT = 10050,
    NFS4ERR_BAD_SESSION_DIGEST = 10051,
    NFS4ERR_BADSESSION = 10052,
    NFS4ERR_BADSLOT = 10053,
    NFS4ERR_COMPLETE_ALREADY = 10054,
    NFS4ERR_CONN_NOT_BOUND_TO_SESSION = 10055,
    NFS4ERR_DELEG_ALREADY_WANTED = 10056,
    NFS4ERR_BACK_CHAN_BUSY = 10057,
    NFS4ERR_LAYOUTTRYLATER = 10058,
    NFS4ERR_LAYOUTUNAVAILABLE = 10059,
    NFS4ERR_NOMATCHING_LAYOUT = 10060,
    NFS4ERR_RECALLCONFLICT = 10061,
    NFS4ERR_UNKNOWN_LAYOUTTYPE = 10062,
    NFS4ERR_SEQ_MISORDERED = 10063,
    NFS4ERR_SEQUENCE_POS = 10064,
    NFS4ERR_REQ_TOO_BIG = 10065,
    NFS4ERR_REP_TOO_BIG = 10066,
    NFS4ERR_REP_TOO_BIG_TO_CACHE = 10067,
    NFS4ERR_RETRY_UNCACHED_REP = 10068,
    NFS4ERR_UNSAFE_COMPOUND = 10069,
    NFS4ERR_TOO_MANY_OPS = 10070,
    NFS4ERR_OP_NOT_IN_SESSION = 10071,
    NFS4ERR_HASH_ALG_UNSUPP = 10072,
    NFS4ERR_CLIENTID_BUSY = 10074,
    NFS4ERR_PNFS_IO_HOLE = 10075,
    NFS4ERR_SEQ_FALSE_RETRY = 10076,
    NFS4ERR_BAD_HIGH_SLOT = 10077,
    NFS4ERR_DEADSESSION = 10078,
    NFS4ERR_ENCR_ALG_UNSUPP = 10079,
    NFS4ERR_PNFS_NO_LAYOUT = 10080,
    NFS4ERR_NOT_ONLY_OP = 10081,
    NFS4ERR_WRONG_CRED = 10082,
    NFS4ERR_WRONG_TYPE = 10083,
    NFS4ERR_DIRDELEG_UNAVAIL = 10084,
    NFS4ERR_REJECT_DELEG = 10085,
    NFS4ERR_RETURNCONFLICT = 10086,
    NFS4ERR_DELEG_REVOKED = 10087,
    NFS4ERR_PARTNER_NOTSUPP = 10088,
    NFS4ERR_PARTNER_NO_AUTH = 10089,
    NFS4ERR_UNION_NOTSUPP = 10090,
    NFS4ERR_OFFLOAD_DENIED = 10091,
    NFS4ERR_WRONG_LFS = 10092,
    NFS4ERR_BADLABEL = 10093,
    NFS4ERR_OFFLOAD_NO_REQS = 10094,
    NFS4ERR_NOXATTR = 10095,
    NFS4ERR_XATTR2BIG = 10096,
    NFS4ERR_REPLAY = 11001,
}

impl xdr::SerializeEnum for nfsstat4 {}
impl xdr::DeserializeEnum for nfsstat4 {}

/// NFS operation numbers as defined in RFC 7530
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
pub enum nfs_opnum4 {
    OP_NULL = 0,
    OP_COMPOUND = 1,
    OP_ACCESS = 3,
    OP_CLOSE = 4,
    OP_COMMIT = 5,
    OP_CREATE = 6,
    OP_DELEGPURGE = 7,
    OP_DELEGRETURN = 8,
    OP_GETATTR = 9,
    OP_GETFH = 10,
    OP_LINK = 11,
    OP_LOCK = 12,
    OP_LOCKT = 13,
    OP_LOCKU = 14,
    OP_LOOKUP = 15,
    OP_LOOKUPP = 16,
    OP_NVERIFY = 17,
    OP_OPEN = 18,
    OP_OPENATTR = 19,
    OP_OPEN_CONFIRM = 20,
    OP_OPEN_DOWNGRADE = 21,
    OP_PUTFH = 22,
    OP_PUTPUBFH = 23,
    OP_PUTROOTFH = 24,
    OP_READ = 25,
    OP_READDIR = 26,
    OP_READLINK = 27,
    OP_REMOVE = 28,
    OP_RENAME = 29,
    OP_RENEW = 30,
    OP_RESTOREFH = 31,
    OP_SAVEFH = 32,
    OP_SECINFO = 33,
    OP_SETATTR = 34,
    OP_SETCLIENTID = 35,
    OP_SETCLIENTID_CONFIRM = 36,
    OP_VERIFY = 37,
    OP_WRITE = 38,
    OP_RELEASE_LOCKOWNER = 39,
    // NFSv4.1 operations
    OP_BACKCHANNEL_CTL = 40,
    OP_BIND_CONN_TO_SESSION = 41,
    OP_EXCHANGE_ID = 42,
    OP_CREATE_SESSION = 43,
    OP_DESTROY_SESSION = 44,
    OP_FREE_STATEID = 45,
    OP_GET_DIR_DELEGATION = 46,
    OP_GETDEVICEINFO = 47,
    OP_GETDEVICELIST = 48,
    OP_LAYOUTCOMMIT = 49,
    OP_LAYOUTGET = 50,
    OP_LAYOUTRETURN = 51,
    OP_SECINFO_NO_NAME = 52,
    OP_SEQUENCE = 53,
    OP_SET_SSV = 54,
    OP_TEST_STATEID = 55,
    OP_WANT_DELEGATION = 56,
    OP_DESTROY_CLIENTID = 57,
    OP_RECLAIM_COMPLETE = 58,
    // NFSv4.2 operations
    OP_ALLOCATE = 59,
    OP_COPY = 60,
    OP_COPY_NOTIFY = 61,
    OP_DEALLOCATE = 62,
    OP_IO_ADVISE = 63,
    OP_LAYOUTERROR = 64,
    OP_LAYOUTSTATS = 65,
    OP_OFFLOAD_CANCEL = 66,
    OP_OFFLOAD_STATUS = 67,
    OP_READ_PLUS = 68,
    OP_SEEK = 69,
    OP_WRITE_SAME = 70,
    OP_CLONE = 71,
    OP_GETXATTR = 72,
    OP_SETXATTR = 73,
    OP_LISTXATTR = 74,
    OP_REMOVEXATTR = 75,
    OP_ILLEGAL = 10044,
}

impl xdr::SerializeEnum for nfs_opnum4 {}
impl xdr::DeserializeEnum for nfs_opnum4 {}

/// NFSv4 filehandle structure (RFC 7530 Section 2.2).
/// An opaque, variable-length byte string used to uniquely identify
/// a filesystem object within a single export. Maximum size is NFS4_FHSIZE (128 bytes).
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
pub struct nfs_fh4 {
    /// Opaque filehandle byte data
    pub data: Vec<u8>, // Maximum size NFS4_FHSIZE (128 bytes)
}

/// NFSv4 state identifier.
/// Uniquely identifies a state object (open, lock, delegation) held by a client
/// on the server. The structure includes sequencing for replay protection.
#[allow(non_camel_case_types)]
#[derive(Default)]
pub struct stateid4 {
    /// Sequence identifier for stateid validation and replay protection (RFC 7530 Section 9.1.4.1)
    seqid: u32,
    /// Opaque identifier bytes (typically 12 bytes for NFSv4.1)
    other: Vec<u8>, // Normally NFS4_STATEID_OTHER_SIZE (12 bytes)
}

/// NFS file type enumeration
#[allow(non_camel_case_types)]
#[derive(Default)]
pub enum nfs_ftype4 {
    #[default]
    NO_FILE_TYPE = 0,
    REGULAR_FILE = 1,
    CHARACTER_FILE = 2,
    BLOCK_FILE = 3,
    SYMBOLIC_LINK = 4,
    SOCKET_FILE = 5,
    FIFO_FILE = 6,
    DIRECTORY = 7,
}

/// Delegation type classification (RFC 7530 Section 10.4)
#[allow(non_camel_case_types)]
pub enum delegation_type4 {
    OPEN_DELEGATE_READ,  // Read delegation (client can cache data safely)
    OPEN_DELEGATE_WRITE, // Write delegation (exclusive write access)
}

/// Classification of state types stored in the global state table.
/// Acts as a tagged union for different state objects.
#[allow(non_camel_case_types)]
#[derive(Default)]
pub enum state_type {
    #[default]
    STATE_TYPE_NONE,
    STATE_TYPE_OPEN(open_state),        // Open file state
    STATE_TYPE_DELEG(delegation_state), // Delegation state
    STATE_TYPE_LOCK(lock_state),        // Byte-range lock state
}

/// Classification of state owner types for ownership validation.
#[allow(non_camel_case_types)]
#[derive(Default)]
pub enum state_owner_type {
    #[default]
    INVALID,
    OPEN(open_owner),      // Owner of OPEN state
    LOCK(lock_owner),      // Owner of LOCK state
    DELEGATION(clientid4), // Delegation owner (the client itself)
}

/// Represents an open file instance
#[allow(non_camel_case_types)]
pub struct open_state {
    stateid: stateid4,
    /// List of lock stateids associated with this open instance
    locks: Vec<stateid4>,
    /// The filehandle this open state refers to
    filehandle: filehandle,
    /// The owner who opened this file
    open_owner: Arc<RwLock<open_owner>>,
    /// Share access modes (READ, WRITE, BOTH) for this open
    share_access: u32,
    /// Share denial modes (READ, WRITE, BOTH) for this open
    share_deny: u32,
    /// Last sequence ID used for operation ordering
    last_sequid: seqid4,
    /// Generation number for state recovery after server reboot
    generation_number: u64,
}

/// NFS lock type classification (RFC 7530 Section 16.12.1)
#[allow(non_camel_case_types)]
enum nfs_lock_type4 {
    READ_LT = 1,   // Shared read lock
    WRITE_LT = 2,  // Exclusive write lock
    READW_LT = 3,  // Blocking read lock
    WRITEW_LT = 4, // Blocking write lock
}

/// Represents a byte-range lock on a file (RFC 7530 Section 16.12)
#[allow(non_camel_case_types)]
pub struct lock_state {
    stateid: stateid4,
    /// The open state this lock is associated with
    open_state: Arc<RwLock<open_state>>,
    /// The specific owner of this lock
    lock_owner: Arc<RwLock<lock_owner>>,
    /// Type of lock (read/write, blocking/non-blocking)
    lock_type: nfs_lock_type4,
    /// Locked byte range (offset, length)
    range: (u64, u64),
    /// The filehandle this lock protects
    filehandle: filehandle,
    /// Last sequence ID used for operation ordering
    last_sequid: seqid4,
    /// Generation number for state recovery
    generation_number: u64,
}

/// Delegation state information (RFC 7530 Section 10)
/// Represents a delegated authority to cache file data locally
#[allow(non_camel_case_types)]
pub struct delegation_state {
    stateid: stateid4,
    /// Associated open state (required for maintaining file access context)
    open_state: Arc<RwLock<open_state>>,
    /// Type of delegation (read or write)
    delegation_type: delegation_type4,
    /// Client information for callback operations
    nfs_client_id: nfs_client_id,
    /// The filehandle being delegated
    filehandle: filehandle,
}

/// Enhanced filehandle with extended attributes and state tracking
#[allow(non_camel_case_types)]
pub struct filehandle {
    /// Filesystem instance this handle belongs to
    fsal: Arc<RwLock<NFSv4FS>>,
    /// Type of the referenced filesystem object
    obj_type: nfs_ftype4,
    /// Base NFS filehandle data
    nfs_fh4: nfs_fh4,
    /// Persistent filesystem-unique identifier
    fileid: u64,
}

/// Client ID confirmation state machine states
#[allow(non_camel_case_types)]
enum nfs_clientid_confirm_state {
    UNCONFIRMED_CLIENT_ID, // Client ID created but not yet confirmed
    CONFIRMED_CLIENT_ID,   // Client ID confirmed and active
    EXPIRED_CLIENT_ID,     // Client ID expired (lease timeout)
    STALE_CLIENT_ID,       // Client ID no longer valid
}

/// Client authentication credentials container
#[allow(non_camel_case_types)]
struct nfs_credentials {
    /// Authentication flavor (UNIX, Kerberos, etc.)
    flavour: u32,
    /// Length of credential data
    length: u32,
    // TODO: Implement proper credential storage based on RFC 7530 Section 3.2
}

/// Callback channel information for server-to-client communication
pub struct CallbackInfo {
    /// Unique callback identifier
    identifier: u32,
    /// Network address for callback connection
    address: SocketAddr,
    /// RPC program number for callback service
    rpc_program: u32,
    /// RPC version number for callback service
    rpc_version: u32, // TODO: Add authentication information for secure callback channels
}

/// Complete client management structure (RFC 7530 Section 9.1.2)
/// Contains all server-side state related to a specific NFS client
#[allow(non_camel_case_types)]
pub struct nfs_client_id {
    clientid: clientid4,
    /// Verifier for client ID establishment
    verifier: [u8; 8],
    /// Previous verifier for state recovery
    last_verifier: [u8; 8],
    /// Lease duration for this client
    lease_duration: Duration,
    /// CURRENT lease expiration time - must be updated on each client request
    lease_time: Instant,
    /// Current confirmation status
    confirm_status: nfs_clientid_confirm_state,
    /// Client authentication credentials
    credentials: nfs_credentials,
    /// All open owners belonging to this client
    open_owners: Vec<Arc<RwLock<open_owner>>>,
    /// All lock owners belonging to this client
    lock_owners: Vec<Arc<RwLock<lock_owner>>>,
    /// Filehandles with active delegations for this client
    delegations: Vec<nfs_fh4>,
    /// All stateids owned by this client
    states: Vec<stateid4>,
    /// Callback channel information for this client
    callback_info: CallbackInfo,
}

/// OPEN owner context (RFC 7530 Section 16.16.2)
/// Represents a specific opener context within a client
#[allow(non_camel_case_types)]
pub struct open_owner {
    /// Client ID of the owner
    clientid: clientid4,
    /// All stateids owned by this open context
    states: Vec<stateid4>,
}

/// LOCK owner context
/// Represents a specific lock owner within an open context
#[allow(non_camel_case_types)]
pub struct lock_owner {
    /// Client ID of the owner
    clientid: clientid4,
    /// The open owner this lock owner belongs to
    open_owner: Arc<RwLock<open_owner>>,
    /// All lock stateids owned by this lock context
    states: Vec<stateid4>,
}
