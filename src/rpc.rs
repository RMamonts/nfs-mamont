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
    command_queue_size: NonZeroUsize,
    result_queue_size: NonZeroUsize,
    max_in_flight_requests: NonZeroUsize,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            read_buffer_size: default_non_zero_usize(4096),
            allocator_buffer_size: default_non_zero_usize(4096),
            allocator_buffer_count: default_non_zero_usize(16),
            command_queue_size: default_non_zero_usize(64),
            result_queue_size: default_non_zero_usize(64),
            max_in_flight_requests: default_non_zero_usize(8),
        }
    }
}

impl ServerSettings {
    /// Creates a settings value with defaults suitable for a small server.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the read buffer size in bytes.
    pub fn read_buffer_size(&self) -> NonZeroUsize {
        self.read_buffer_size
    }

    /// Sets the read buffer size in bytes.
    pub fn with_read_buffer_size(mut self, read_buffer_size: NonZeroUsize) -> Self {
        self.read_buffer_size = read_buffer_size;
        self
    }

    /// Returns the allocator buffer size in bytes.
    pub fn allocator_buffer_size(&self) -> NonZeroUsize {
        self.allocator_buffer_size
    }

    /// Sets the allocator buffer size in bytes.
    pub fn with_allocator_buffer_size(mut self, allocator_buffer_size: NonZeroUsize) -> Self {
        self.allocator_buffer_size = allocator_buffer_size;
        self
    }

    /// Returns the number of allocator buffers.
    pub fn allocator_buffer_count(&self) -> NonZeroUsize {
        self.allocator_buffer_count
    }

    /// Sets the number of allocator buffers.
    pub fn with_allocator_buffer_count(mut self, allocator_buffer_count: NonZeroUsize) -> Self {
        self.allocator_buffer_count = allocator_buffer_count;
        self
    }

    /// Returns the maximum number of queued RPC commands per connection.
    pub fn command_queue_size(&self) -> NonZeroUsize {
        self.command_queue_size
    }

    /// Sets the maximum number of queued RPC commands per connection.
    pub fn with_command_queue_size(mut self, command_queue_size: NonZeroUsize) -> Self {
        self.command_queue_size = command_queue_size;
        self
    }

    /// Returns the maximum number of queued RPC replies per connection.
    pub fn result_queue_size(&self) -> NonZeroUsize {
        self.result_queue_size
    }

    /// Sets the maximum number of queued RPC replies per connection.
    pub fn with_result_queue_size(mut self, result_queue_size: NonZeroUsize) -> Self {
        self.result_queue_size = result_queue_size;
        self
    }

    /// Returns the maximum number of concurrently processed RPC requests per connection.
    pub fn max_in_flight_requests(&self) -> NonZeroUsize {
        self.max_in_flight_requests
    }

    /// Sets the maximum number of concurrently processed RPC requests per connection.
    pub fn with_max_in_flight_requests(mut self, max_in_flight_requests: NonZeroUsize) -> Self {
        self.max_in_flight_requests = max_in_flight_requests;
        self
    }
}

fn default_non_zero_usize(value: usize) -> NonZeroUsize {
    match NonZeroUsize::new(value) {
        Some(value) => value,
        None => unreachable!("default server setting must be non-zero"),
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
    /// Creates a new context with the provided settings and no backend.
    pub fn with_settings(settings: ServerSettings) -> Self {
        Self { settings, ..Self::default() }
    }

    /// Creates a new context with the provided backend.
    pub fn with_backend(backend: SharedVfs) -> Self {
        Self { backend: Some(backend), ..Self::default() }
    }

    /// Creates a new context with the provided backend and settings.
    pub fn with_backend_and_settings(backend: SharedVfs, settings: ServerSettings) -> Self {
        Self { settings, backend: Some(backend), ..Self::default() }
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

#[cfg(test)]
mod tests {
    use super::ServerSettings;

    #[test]
    fn server_settings_allow_queue_configuration() {
        let settings = ServerSettings::new()
            .with_command_queue_size(std::num::NonZeroUsize::new(8).expect("non-zero literal"))
            .with_result_queue_size(std::num::NonZeroUsize::new(16).expect("non-zero literal"))
            .with_max_in_flight_requests(std::num::NonZeroUsize::new(4).expect("non-zero literal"));

        assert_eq!(settings.command_queue_size().get(), 8);
        assert_eq!(settings.result_queue_size().get(), 16);
        assert_eq!(settings.max_in_flight_requests().get(), 4);
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
