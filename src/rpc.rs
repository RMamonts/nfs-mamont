use std::io;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::string::FromUtf8Error;
use std::sync::Arc;

use num_derive::{FromPrimitive, ToPrimitive};
use tokio::sync::RwLock;

use crate::allocator::Slice;
use crate::parser::Arguments;
use crate::vfs;
use crate::vfs::file;

pub const RPC_VERSION: u32 = 2;

pub const MAX_AUTH_SIZE: usize = 400;

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

#[derive(ToPrimitive, FromPrimitive)]
pub enum RpcBody {
    Call = 0,
    Reply = 1,
}

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

#[derive(Debug, Clone)]
pub struct OpaqueAuth {
    pub flavor: AuthFlavor,
    pub body: Vec<u8>,
}

pub type SharedVfs = Arc<dyn vfs::Vfs + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub struct ServerSettings {
    read_buffer_size: NonZeroUsize,
    allocator_buffer_size: NonZeroUsize,
    allocator_buffer_count: NonZeroUsize,
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

impl ServerSettings {
    /// Returns the read buffer size in bytes.
    pub fn read_buffer_size(&self) -> NonZeroUsize {
        self.read_buffer_size
    }

    /// Returns the allocator buffer size in bytes.
    pub fn allocator_buffer_size(&self) -> NonZeroUsize {
        self.allocator_buffer_size
    }

    /// Returns the number of allocator buffers.
    pub fn allocator_buffer_count(&self) -> NonZeroUsize {
        self.allocator_buffer_count
    }
}

#[derive(Debug, Clone)]
pub struct ServerExport {
    directory: file::Path,
    allowed_hosts: Vec<String>,
}

#[derive(Clone)]
pub struct ServerContext {
    settings: ServerSettings,
    backend: Option<SharedVfs>,
    exports: Arc<RwLock<Vec<ServerExport>>>,
    mounts: Arc<RwLock<Vec<MountedDirectory>>>,
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
    /// Creates a new context with the provided backend.
    pub fn with_backend(backend: SharedVfs) -> Self {
        Self { backend: Some(backend), ..Self::default() }
    }

    /// Returns the server settings.
    pub fn settings(&self) -> &ServerSettings {
        &self.settings
    }

    /// Returns the configured backend, if any.
    pub fn backend(&self) -> Option<&SharedVfs> {
        self.backend.as_ref()
    }

    /// Registers an exported directory.
    pub async fn add_export(&self, export: ServerExport) {
        self.exports.write().await.push(export);
    }

    /// Returns a snapshot of configured exports.
    pub async fn exports(&self) -> Vec<ServerExport> {
        self.exports.read().await.clone()
    }

    /// Records a mounted directory for a client.
    pub async fn record_mount(&self, client_addr: String, directory: file::Path) {
        self.mounts.write().await.push(MountedDirectory { client_addr, directory });
    }

    /// Returns a snapshot of active mounts.
    pub async fn mount_entries(&self) -> Vec<(String, file::Path)> {
        self.mounts
            .read()
            .await
            .iter()
            .map(|mount| (mount.client_addr.clone(), mount.directory.clone()))
            .collect()
    }

    /// Removes a single mounted directory for a client.
    pub async fn remove_mount(&self, client_addr: &str, directory: &file::Path) {
        self.mounts.write().await.retain(|mount| {
            !(mount.client_addr == client_addr && mount.directory.as_path() == directory.as_path())
        });
    }

    /// Removes all mounted directories associated with a client.
    pub async fn remove_mounts_by_client(&self, client_addr: &str) {
        self.mounts.write().await.retain(|mount| mount.client_addr != client_addr);
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionContext {
    local_addr: Option<SocketAddr>,
    client_addr: Option<SocketAddr>,
    auth: Option<AuthFlavor>,
}

impl ConnectionContext {
    /// Creates a new connection context.
    pub fn new(local_addr: Option<SocketAddr>, client_addr: Option<SocketAddr>) -> Self {
        Self { local_addr, client_addr, auth: None }
    }

    /// Returns the local socket address.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    /// Returns the client socket address.
    pub fn client_addr(&self) -> Option<SocketAddr> {
        self.client_addr
    }

    /// Returns the negotiated authentication flavor.
    pub fn auth(&self) -> Option<AuthFlavor> {
        self.auth
    }

    fn with_auth(mut self, auth: AuthFlavor) -> Self {
        self.auth = Some(auth);
        self
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
            context: RequestContext { connection: connection.with_auth(auth), header: self.header },
            arguments: self.arguments,
        }
    }
}

impl ServerExport {
    /// Creates a new export definition.
    pub fn new(directory: file::Path, allowed_hosts: Vec<String>) -> Self {
        Self { directory, allowed_hosts }
    }

    /// Returns the exported directory.
    pub fn directory(&self) -> &file::Path {
        &self.directory
    }

    /// Returns the allowed host patterns.
    pub fn allowed_hosts(&self) -> &[String] {
        &self.allowed_hosts
    }
}

#[derive(Debug, Clone)]
struct MountedDirectory {
    client_addr: String,
    directory: file::Path,
}

pub struct RpcCommand {
    pub context: RequestContext,
    pub arguments: Box<Arguments>,
}

pub enum ReplyPayload {
    Buffer(Vec<u8>),
    Read { header: Vec<u8>, data: Slice, padding: usize },
}

pub struct RpcReply {
    pub xid: u32,
    pub payload: ReplyPayload,
}

impl RpcReply {
    pub fn new(xid: u32, payload: ReplyPayload) -> Self {
        Self { xid, payload }
    }
}

pub type CommandResult = io::Result<RpcReply>;

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
