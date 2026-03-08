use std::io;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::string::FromUtf8Error;
use std::sync::{Arc, RwLock};

use num_derive::{FromPrimitive, ToPrimitive};

use crate::parser::Arguments;
use crate::vfs;
use crate::vfs::file;

pub const RPC_VERSION: u32 = 2;

pub const MAX_AUTH_SIZE: usize = 400;

#[allow(dead_code)]
#[derive(ToPrimitive, FromPrimitive)]
pub enum AcceptStat {
    Success = 0,
    ProgUnavail = 1,
    ProgMismatch = 2,
    ProcUnavail = 3,
    GarbageArgs = 4,
    SystemErr = 5,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ToPrimitive, FromPrimitive)]
pub enum AuthStat {
    Ok = 0,
    BadCred = 1,
    RejectedCred = 2,
    BadVerf = 3,
    RejectedVerf = 4,
    TooWeak = 5,
    InvalidResp = 6,
    Failed = 7,
    KerbGeneric = 8,
    TimeExpire = 9,
    TktFile = 10,
    Decode = 11,
    NetAddr = 12,
    RpcSecGssCredProblem = 13,
    RpcSecGssCtxProblem = 14,
}

#[allow(dead_code)]
#[derive(ToPrimitive, FromPrimitive)]
pub enum RpcBody {
    Call = 0,
    Reply = 1,
}

#[allow(dead_code)]
pub enum ReplyBody {
    MsgAccepted = 0,
    MsgDenied = 1,
}

/// Authentication flavors.
#[derive(Debug, Copy, Clone, PartialEq, Eq, ToPrimitive, FromPrimitive)]
pub enum AuthFlavor {
    None = 0,
    Sys = 1,
    Short = 2,
    Dh = 3,
    RpcSecGss = 6,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OpaqueAuth {
    pub flavor: AuthFlavor,
    pub body: Vec<u8>,
}

pub type SharedVfs = Arc<dyn vfs::Vfs + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub struct ServerSettings {
    pub read_buffer_size: NonZeroUsize,
    pub allocator_buffer_size: NonZeroUsize,
    pub allocator_buffer_count: NonZeroUsize,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            read_buffer_size: NonZeroUsize::new(4096).expect("read buffer size must be non-zero"),
            allocator_buffer_size: NonZeroUsize::new(4096)
                .expect("allocator buffer size must be non-zero"),
            allocator_buffer_count: NonZeroUsize::new(16)
                .expect("allocator buffer count must be non-zero"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerExport {
    pub directory: file::Path,
    pub allowed_hosts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ServerMount {
    pub client_addr: String,
    pub directory: file::Path,
}

#[derive(Clone)]
pub struct ServerContext {
    pub settings: ServerSettings,
    pub backend: Option<SharedVfs>,
    pub exports: Arc<RwLock<Vec<ServerExport>>>,
    pub mounts: Arc<RwLock<Vec<ServerMount>>>,
}

impl Default for ServerContext {
    fn default() -> Self {
        Self {
            settings: ServerSettings::default(),
            backend: None,
            exports: Arc::new(RwLock::new(Vec::new())),
            mounts: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl ServerContext {
    pub fn with_backend(backend: SharedVfs) -> Self {
        Self { backend: Some(backend), ..Self::default() }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionContext {
    pub local_addr: Option<SocketAddr>,
    pub client_addr: Option<SocketAddr>,
    pub auth: Option<AuthFlavor>,
}

impl ConnectionContext {
    pub fn new(local_addr: Option<SocketAddr>, client_addr: Option<SocketAddr>) -> Self {
        Self { local_addr, client_addr, auth: None }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RpcCallHeader {
    pub xid: u32,
    pub program: u32,
    pub version: u32,
    pub procedure: u32,
    pub auth_flavor: AuthFlavor,
}

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub connection: ConnectionContext,
    pub header: RpcCallHeader,
}

pub struct ParsedRpcCall {
    pub header: RpcCallHeader,
    pub arguments: Box<Arguments>,
}

impl ParsedRpcCall {
    pub fn with_connection(self, connection: ConnectionContext) -> RpcCommand {
        let auth = self.header.auth_flavor;
        RpcCommand {
            context: RequestContext {
                connection: ConnectionContext { auth: Some(auth), ..connection },
                header: self.header,
            },
            arguments: self.arguments,
        }
    }
}

pub struct RpcCommand {
    pub context: RequestContext,
    pub arguments: Box<Arguments>,
}

#[derive(Debug, Clone)]
pub struct RpcReply {
    pub xid: u32,
    pub payload: Vec<u8>,
}

impl RpcReply {
    pub fn new(xid: u32, payload: Vec<u8>) -> Self {
        Self { xid, payload }
    }
}

pub type CommandResult = io::Result<RpcReply>;

#[allow(dead_code)]
pub enum RejectedReply {
    RpcMismatch = 0,
    AuthError = 1,
}

/// Represents a mismatch in program/protocol versions.
/// Returns highest and lowest versions of available versions of requested program
#[derive(Debug)]
pub struct VersionMismatch {
    pub low: u32,
    pub high: u32,
}

/// Errors that can occur during parsing.
#[derive(Debug)]
pub enum Error {
    /// The maximum element limit was exceeded.
    MaxElemLimit,
    /// An I/O error occurred.
    IO(io::Error),
    /// An enum discriminant mismatch occurred.
    EnumDiscMismatch,
    /// An incorrect string was encountered during UTF-8 conversion.
    IncorrectString(FromUtf8Error),
    /// Incorrect padding was found.
    IncorrectPadding,
    /// An impossible type cast was attempted.
    ImpossibleTypeCast,
    /// A bad file handle was encountered.
    BadFileHandle,
    /// A message type mismatch occurred.
    MessageTypeMismatch,
    /// An RPC version mismatch occurred.
    RpcVersionMismatch(VersionMismatch),
    /// An authentication error occurred.
    AuthError(AuthStat),
    /// A program mismatch occurred.
    ProgramMismatch,
    /// A procedure mismatch occurred.
    ProcedureMismatch,
    /// A program version mismatch occurred.
    ProgramVersionMismatch(VersionMismatch),
}
