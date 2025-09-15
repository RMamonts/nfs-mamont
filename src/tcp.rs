//! The TCP module provides functionality for handling NFS protocol communications over TCP.
//!
//! This module implements a TCP listener for NFS server that:
//! - Handles connections from NFS clients
//! - Processes RPC messages received over TCP
//! - Manages connection lifecycle and message framing
//! - Provides interface for mounting and unmounting file systems
//!
//! The implementation supports configurable export paths and notification
//! on mount/unmount operations.
use async_trait::async_trait;
use dashmap::DashMap;
use std::collections::HashSet;
use std::io::{Cursor, Read, Write};
use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;
use std::{io, net::IpAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, trace, warn};

use crate::protocol::nfs::portmap::PortmapTable;
use crate::protocol::rpc::Context;
use crate::protocol::xdr::{mount, portmap};
use crate::protocol::{nfs, rpc, xdr};
use crate::utils::error::io_other;
use crate::vfs::NFSFileSystem;
use crate::xdr::{deserialize, nfs3, Serialize};

/// Default transaction retention period
const TRANSACTION_RETENTION_PERIOD: Duration = Duration::from_secs(60);
const MAX_RM_FRAGMENT_SIZE: usize = 2147483647;
const LAST_FG_MASK: u32 = 2147483648;
const COMMAND_INIT_SIZE: usize = 8192;
/// RPC program number for NFS Access Control Lists
const NFS_ACL_PROGRAM: u32 = 100227;
/// RPC program number for NFS ID Mapping
const NFS_ID_MAP_PROGRAM: u32 = 100270;
/// RPC program number for LOCALIO auxiliary RPC protocol
/// More about <https://docs.kernel.org/filesystems/nfs/localio.html#rpc.>
const NFS_LOCALIO_PROGRAM: u32 = 400122;
/// RPC program number for NFS Metadata
const NFS_METADATA_PROGRAM: u32 = 200024;
/// Initial size of RPC response buffer
const DEFAULT_RESPONSE_BUFFER_CAPACITY: usize = 8192;

/// Entry in the NFS export table that represents a single exported file system.
pub struct NFSExportTableEntry {
    /// Arc reference to the NFS file system implementation
    pub vfs: Arc<dyn NFSFileSystem + Send + Sync + 'static>,
    /// Channel for mount/unmount notifications
    /// If `Some(sender)`, it will be used to notify when a client mounts (`true`) or unmounts (`false`) the file system.
    pub mount_signal: Option<mpsc::Sender<bool>>,
    /// Name of the exported file system path
    pub export_name: String,
}

/// Hash map that stores all exported file systems for an NFS server.
pub type NFSExportTable = DashMap<nfs3::fs_id, NFSExportTableEntry>;

/// NFS TCP Connection Handler that listens for incoming NFS client connections
/// and processes RPC messages over TCP transport.
pub struct NFSTcpListener {
    next_id: AtomicU64,
    /// TCP Listener for accepting incoming connections
    listener: TcpListener,
    /// Port on which the server is listening
    port: u16,
    /// Table of NFS exports managed by this server
    export_table: Arc<NFSExportTable>,
    /// Tracker for RPC transactions to handle retransmissions
    transaction_tracker: Arc<rpc::TransactionTracker>,
    /// Portmap table storing port-to-program mappings
    /// (like a portmap service)
    portmap_table: Arc<RwLock<PortmapTable>>,
    /// List of connected clients and file systems they have mounted
    client_list: Arc<DashMap<String, HashSet<String>>>,
}

/// Generates a local loopback IP address from a 16-bit host number
/// Used for creating multiple local test addresses in the 127.88.x.y range
pub fn generate_host_ip(hostnum: u16) -> String {
    format!("127.88.{}.{}", ((hostnum >> 8) & 0xFF) as u8, (hostnum & 0xFF) as u8)
}

/// RPC command type with context
#[derive(Debug)]
pub struct RpcCommand {
    /// RPC message data
    data: Vec<u8>,
}

fn parse_header(arg: u32) -> (bool, usize) {
    (arg & (1 << 31) > 0, arg as usize & MAX_RM_FRAGMENT_SIZE)
}

impl RpcCommand {
    pub async fn read_command_from_socket(
        &mut self,
        socket: &mut ReadHalf<TcpStream>,
    ) -> io::Result<()> {
        let mut header_buf = [0_u8; 4];
        let mut start_offset = 0;
        loop {
            socket.read_exact(&mut header_buf).await?;
            let fragment_header = u32::from_be_bytes(header_buf);
            let (is_last, length) = parse_header(fragment_header);
            debug!("Reading fragment length:{}, last:{}", length, is_last);
            self.data.resize(self.data.len() + length, 0);
            socket.read_exact(&mut self.data[start_offset..]).await?;
            debug!("Finishing Reading fragment length:{}, last:{}", length, is_last);
            if is_last {
                break;
            }
            start_offset += length;
        }
        Ok(())
    }
}

/// Represents a response buffer that minimizes data copying
pub struct ResponseBuffer {
    /// Internal buffer for writing data
    buffer: Vec<u8>,
    /// Indicates that the buffer contains data to send
    has_content: bool,
}

impl ResponseBuffer {
    /// Creates a new response buffer with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self { buffer: Vec::with_capacity(capacity), has_content: false }
    }

    /// Gets the internal buffer for writing
    pub fn get_mut_buffer(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    /// Marks the buffer as containing data to send
    pub fn mark_has_content(&mut self) {
        self.has_content = true;
    }

    /// Checks if the buffer contains data to send
    pub fn has_content(&self) -> bool {
        self.has_content
    }

    /// Takes the internal buffer, consuming the structure
    pub fn into_inner(self) -> Vec<u8> {
        self.buffer
    }

    /// Clears the buffer for reuse
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.has_content = false;
    }
    pub async fn write_fragment(
        &mut self,
        write_half: &mut WriteHalf<TcpStream>,
    ) -> io::Result<()> {
        // Maximum fragment size is 2^31 - 1 bytes

        let mut offset = 0;
        while offset < self.buffer.len() {
            // Calculate the size of this fragment
            let remaining = self.buffer.len() - offset;
            let fragment_size = std::cmp::min(remaining, MAX_RM_FRAGMENT_SIZE);

            // Determine if this is the last fragment
            let is_last = offset + fragment_size >= self.buffer.len();

            // Create the fragment header
            // The highest bit indicates if this is the last fragment
            let fragment_header =
                if is_last { fragment_size as u32 + LAST_FG_MASK } else { fragment_size as u32 };

            let header_buf = u32::to_be_bytes(fragment_header);
            write_half.write_all(&header_buf).await?;

            trace!("Writing fragment length:{}, last:{}", fragment_size, is_last);
            write_half.write_all(&self.buffer[offset..offset + fragment_size]).await?;

            offset += fragment_size;
        }

        Ok(())
    }
}

/// Command processing result
pub type CommandResult = Result<Option<ResponseBuffer>, io::Error>;

/// Processes an established TCP socket connection from an NFS client
///
/// This function:
/// - Creates an RPC message handler for the socket
/// - Sets up asynchronous message processing
/// - Handles bidirectional communication between client and server
/// - Processes incoming RPC requests and sends responses
///
/// # Arguments
///
/// * `socket` - The established TCP connection to the client
/// * `context` - RPC context containing server state and client information
async fn process_socket(socket: TcpStream, context: Context) {
    let (readhalf, writehalf) = tokio::io::split(socket);
    //channel for result
    let (result_sender, result_receiver) = mpsc::unbounded_channel::<CommandResult>();
    //channel for request
    let (command_sender, command_receiver) = mpsc::unbounded_channel::<RpcCommand>();

    // task which reads commands from socket and send them to task, that process them
    spawn_reader_task(readhalf, command_sender);

    //task, that gets result from processor task and writes it into socket
    spawn_writer_task(writehalf, result_receiver);

    //task, that processes command
    spawn_processor_task(command_receiver, result_sender, context);
}

fn spawn_reader_task(
    mut readhalf: ReadHalf<TcpStream>,
    command_sender: UnboundedSender<RpcCommand>,
) {
    tokio::spawn(async move {
        loop {
            let mut command = RpcCommand { data: Vec::with_capacity(COMMAND_INIT_SIZE) };
            match command.read_command_from_socket(&mut readhalf).await {
                Ok(()) => {
                    //here some processing - actually sending to processing rpc task
                    match command_sender.send(command) {
                        Ok(_) => continue,
                        Err(_) => {
                            error!("Failed to submit command to queue");
                            return io_other("Command queue error");
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    return if command.data.is_empty() {
                        trace!("Connection closed before receiving any data");
                        Ok(())
                    } else {
                        error!("Connection closed during command transmission");
                        io_other("Early socket closing")
                    }
                }
                Err(e) => {
                    error!("Message loop broken due to {:?}", e);
                    return Err(e);
                }
            }
        }
    });
}

fn spawn_writer_task(
    mut writehalf: WriteHalf<TcpStream>,
    mut result_receiver: UnboundedReceiver<CommandResult>,
) {
    //task to write to socket
    tokio::spawn(async move {
        while let Some(result) = result_receiver.recv().await {
            match result {
                Ok(Some(mut response_buffer)) if response_buffer.has_content() => {
                    if let Err(e) = response_buffer.write_fragment(&mut writehalf).await {
                        error!("Write error {:?}", e);
                    }
                }
                Ok(None) => {
                    // No response needed, so nothing to send
                }
                Ok(Some(_)) => {
                    // Buffer exists but contains no data to send
                }
                Err(e) => {
                    debug!("Message handling closed : {:?}", e);
                    return Err(e);
                }
            }
        }
        debug!("Command result handler finished");
        Ok(())
    });
}

fn spawn_processor_task(
    mut command_receiver: UnboundedReceiver<RpcCommand>,
    result_sender: UnboundedSender<CommandResult>,
    mut context: Context,
) {
    tokio::spawn(async move {
        // Create reusable buffer for responses
        let mut output_buffer = ResponseBuffer::with_capacity(DEFAULT_RESPONSE_BUFFER_CAPACITY);

        while let Some(command) = command_receiver.recv().await {
            trace!("Processing command from queue");

            // Clear buffer for reuse
            output_buffer.clear();
            // Call async processor
            let result =
                match process_rpc_command(command.data, &mut output_buffer, &mut context).await {
                    Ok(true) => {
                        // Processor indicated response needs to be sent
                        output_buffer.mark_has_content();
                        let buffer_to_send = std::mem::replace(
                            &mut output_buffer,
                            ResponseBuffer::with_capacity(DEFAULT_RESPONSE_BUFFER_CAPACITY),
                        );
                        Ok(Some(buffer_to_send))
                    }
                    Ok(false) => {
                        // No response needed (e.g. retransmission)
                        Ok(None)
                    }
                    Err(e) => Err(e),
                };

            // Send result
            if let Err(e) = result_sender.send(result) {
                error!("Failed to send command processing result: {:?}", e);
                break;
            }
        }
        debug!("Command queue handler finished");
    });
}

/// Interface for NFS TCP servers that defines common operations
/// for managing and interacting with NFS clients over TCP connections.
///
/// This trait provides methods for:
/// - Getting information about the listening socket
/// - Setting up mount event notifications
/// - Starting the server to process client connections
#[async_trait]
pub trait NFSTcp: Send + Sync {
    /// Returns the actual port number on which the server is listening
    ///
    /// This is especially useful when binding to port 0, which allows the OS
    /// to assign any available port. After binding, this method can be used
    /// to determine which port was actually assigned.
    fn get_listen_port(&self) -> u16;

    /// Returns the IP address on which the server is listening
    ///
    /// This is useful when the server binds to a wildcard address (0.0.0.0 or ::)
    /// or when using the "auto" IP address feature, to determine the actual
    /// network interface being used.
    fn get_listen_ip(&self) -> IpAddr;

    /// Registers a channel to receive notifications about mount and unmount events
    ///
    /// # Arguments
    ///
    /// * `signal` - MPSC sender that will receive boolean values:
    ///   * `true` when a client mounts the file system
    ///   * `false` when a client unmounts the file system
    ///
    /// # Returns
    /// `io::Result<()>` indicating:
    /// - `Ok(())` on successful operation
    /// - `Err` if the export ID is not found in the export table
    async fn set_mount_listener(
        &mut self,
        fs_id: xdr::nfs3::fs_id,
        signal: mpsc::Sender<bool>,
    ) -> io::Result<()>;

    /// Starts the NFS server and processes client connections
    ///
    /// This method:
    /// - Accepts incoming TCP connections from NFS clients
    /// - Creates a new RPC context for each connection
    /// - Spawns an asynchronous task to handle each connection
    /// - Continues accepting connections indefinitely
    ///
    /// This method runs in an infinite loop and only returns if there's an error
    /// with the underlying TCP listener.
    async fn handle_forever(&self) -> io::Result<()>;
}

impl NFSTcpListener {
    /// Creates a new NFS TCP listener bound to the specified IP address and port
    ///
    /// # Arguments
    ///
    /// * `ipstr` - IP address and port in the format "IP:PORT" (e.g. "127.0.0.1:2049")
    ///   Special value "auto:PORT" attempts to find an available local address
    ///
    /// # Returns
    ///
    /// A Result containing either the new [`NFSTcpListener`] or an IO error
    pub async fn bind(ipstr: &str) -> io::Result<NFSTcpListener> {
        let (ip, port) = ipstr.split_once(':').ok_or_else(|| {
            io::Error::new(io::ErrorKind::AddrNotAvailable, "IP Address must be of form ip:port")
        })?;
        let port = port.parse::<u16>().map_err(|_| {
            io::Error::new(io::ErrorKind::AddrNotAvailable, "Port not in range 0..=65535")
        })?;

        if ip != "auto" {
            return NFSTcpListener::bind_internal(ip, port).await;
        }

        const NUM_TRIES: u16 = 32;
        for try_ip in 1..=NUM_TRIES {
            let ip = generate_host_ip(try_ip);
            let result = NFSTcpListener::bind_internal(&ip, port).await;

            if result.is_ok() {
                return result;
            }
        }

        Err(io::Error::other("Can't bind automatically"))
    }

    /// Internal method to bind the TCP listener to a specific IP and port
    ///
    /// # Arguments
    ///
    /// * `ip` - IP address to bind to
    /// * `port` - Port number to bind to
    async fn bind_internal(ip: &str, port: u16) -> io::Result<NFSTcpListener> {
        let ipstr = format!("{ip}:{port}");
        let listener = TcpListener::bind(&ipstr).await?;
        info!("Listening on {:?}", &ipstr);

        let port = match listener.local_addr()? {
            SocketAddr::V4(s) => s.port(),
            SocketAddr::V6(s) => s.port(),
        };

        Ok(NFSTcpListener {
            next_id: AtomicU64::new(0),
            listener,
            port,
            export_table: Arc::new(DashMap::new()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(
                TRANSACTION_RETENTION_PERIOD,
            )),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
            client_list: Arc::new(DashMap::new()),
        })
    }

    /// Registers a new NFS file system export. Export name defaults to "/".
    ///
    /// # Arguments
    ///
    /// * `fs` - Implementation of the NFSFileSystem trait that will handle NFS operations
    ///
    /// # Returns
    ///
    /// A Result containing either the generated filesystem ID or an error if registration fails
    pub async fn register_root_export<T>(&mut self, fs: T) -> io::Result<xdr::nfs3::fs_id>
    where
        T: NFSFileSystem + Send + Sync + 'static,
    {
        self.register_export_with_name(fs, "").await
    }

    /// Registers a new NFS file system export with a custom export name
    ///
    /// The export name defines the path that clients will use to mount the file system.
    /// This method normalizes the provided name by adding a leading slash and removing
    /// any trailing slashes.
    ///
    /// # Arguments
    ///
    /// * `fs` - Implementation of the NFSFileSystem trait that will handle NFS operations
    /// * `export_name` - The name/path for this export (e.g., "data" becomes "/data")
    ///
    /// # Returns
    ///
    /// A Result containing either the generated filesystem ID or an error if:
    /// - The export name already exists
    /// - Registration fails for another reason
    pub async fn register_export_with_name<T, S>(
        &mut self,
        fs: T,
        export_name: S,
    ) -> io::Result<xdr::nfs3::fs_id>
    where
        T: NFSFileSystem + Send + Sync + 'static,
        S: AsRef<str>,
    {
        let fs_id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let export_name = format!("/{}", export_name.as_ref().trim_matches('/'));

        if self.export_table.iter().any(|entry| entry.export_name == export_name) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Export with name '{export_name}' already exists"),
            ));
        }

        debug!("Registering export with ID {fs_id} and name {export_name}");
        self.export_table.insert(
            fs_id,
            NFSExportTableEntry { vfs: Arc::new(fs), export_name, mount_signal: None },
        );

        Ok(fs_id)
    }

    /// Unregisters an existing NFS file system export by its filesystem ID
    ///
    /// # Arguments
    ///
    /// * `fs_id` - The filesystem ID of the export to remove
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Export was successfully removed
    /// * `Err(io::Error)` - Export with the given ID was not found
    pub fn unregister_export(&mut self, fs_id: xdr::nfs3::fs_id) -> io::Result<()> {
        if self.export_table.remove(&fs_id).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Export with ID '{fs_id}' not found"),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl NFSTcp for NFSTcpListener {
    /// Returns the actual port number on which the server is listening
    ///
    /// This is especially useful when binding to port 0, which allows the OS
    /// to assign any available port. After binding, this method can be used
    /// to determine which port was actually assigned.
    fn get_listen_port(&self) -> u16 {
        let addr = self.listener.local_addr().unwrap();
        addr.port()
    }

    /// Returns the IP address on which the server is listening
    ///
    /// This is useful when the server binds to a wildcard address (0.0.0.0 or ::)
    /// or when using the "auto" IP address feature, to determine the actual
    /// network interface being used.
    fn get_listen_ip(&self) -> IpAddr {
        let addr = self.listener.local_addr().unwrap();
        addr.ip()
    }

    /// Registers a channel to receive notifications about mount and unmount events
    ///
    /// # Arguments
    ///
    /// * `signal` - MPSC sender that will receive boolean values:
    ///   * `true` when a client mounts the file system
    ///   * `false` when a client unmounts the file system
    ///
    /// # Returns
    /// `io::Result<()>` indicating:
    /// - `Ok(())` on successful operation
    /// - `Err` if the export ID is not found in the export table
    async fn set_mount_listener(
        &mut self,
        fs_id: xdr::nfs3::fs_id,
        signal: mpsc::Sender<bool>,
    ) -> io::Result<()> {
        self.export_table.get_mut(&fs_id).ok_or(io::ErrorKind::NotFound)?.mount_signal =
            Some(signal);
        Ok(())
    }

    /// Starts the NFS server and processes client connections
    ///
    /// This method:
    /// - Accepts incoming TCP connections from NFS clients
    /// - Creates a new RPC context for each connection
    /// - Spawns an asynchronous task to handle each connection
    /// - Continues accepting connections indefinitely
    ///
    /// This method runs in an infinite loop and only returns if there's an error
    /// with the underlying TCP listener.
    async fn handle_forever(&self) -> io::Result<()> {
        loop {
            let (socket, _) = self.listener.accept().await?;
            let context = rpc::Context {
                local_port: self.port,
                client_addr: socket.peer_addr()?.to_string(),
                auth: Some(xdr::rpc::auth_unix::default()),
                export_table: self.export_table.clone(),
                transaction_tracker: self.transaction_tracker.clone(),
                portmap_table: self.portmap_table.clone(),
                client_list: self.client_list.clone(),
            };
            info!("Accepting connection from {}", context.client_addr);
            debug!("Accepting socket {:?} {:?}", socket, context);
            socket.set_nodelay(true)?;
            process_socket(socket, context).await;
        }
    }
}

/// Processes a single RPC message
///
/// This function forms the core of the RPC message dispatcher. It:
/// 1. Deserializes the incoming RPC message using XDR format
/// 2. Validates the RPC version number (must be version 2)
/// 3. Extracts authentication information if provided
/// 4. Checks for retransmissions to ensure idempotent operation
/// 5. Routes the call to the appropriate protocol handler (NFS, MOUNT, PORTMAP)
/// 6. Tracks transaction completion state
///
/// This implementation follows RFC 5531 (previously RFC 1057) section on Authentication and
/// Record Marking Standard for proper RPC message handling.
///
/// Returns true if a response was sent, false otherwise (for retransmissions).
pub async fn handle_rpc(
    input: &mut impl Read,
    output: &mut impl Write,
    context: &mut Context,
) -> io::Result<bool> {
    let recv = deserialize::<xdr::rpc::rpc_msg>(input)?;
    let xid = recv.xid;
    if let xdr::rpc::rpc_body::CALL(call) = recv.body {
        if let xdr::rpc::auth_flavor::AUTH_SYS = call.cred.flavor {
            let auth = deserialize(&mut Cursor::new(&call.cred.body))?;
            context.auth = Some(auth);
        }
        if call.rpcvers != xdr::rpc::PROTOCOL_VERSION {
            warn!("Invalid RPC version {} != 2", call.rpcvers);
            xdr::rpc::rpc_vers_mismatch(xid).serialize(output)?;
            return Ok(true);
        }

        if context.transaction_tracker.is_retransmission(xid, &context.client_addr) {
            debug!(
                "Retransmission detected, xid: {}, client_addr: {}, call: {:?}",
                xid, context.client_addr, call
            );
            return Ok(false);
        }

        let result = match call.prog {
            nfs3::PROGRAM => match call.vers {
                nfs3::VERSION => nfs::v3::handle_nfs(xid, call, input, output, context).await,
                nfs::v4::VERSION => nfs::v4::handle_nfs(xid, call, input, output, context).await,
                v => {
                    warn!("Unsupported NFS version: {}", v);
                    xdr::rpc::prog_version_range_mismatch_reply_message(
                        xid,
                        nfs3::VERSION,
                        nfs::v4::VERSION,
                    )
                    .serialize(output)?;
                    Ok(())
                }
            },
            portmap::PROGRAM => {
                nfs::portmap::handle_portmap(xid, &call, input, output, context).await
            }
            mount::PROGRAM => nfs::mount::handle_mount(xid, call, input, output, context).await,
            prog if prog == NFS_ACL_PROGRAM
                || prog == NFS_ID_MAP_PROGRAM
                || prog == NFS_METADATA_PROGRAM =>
            {
                trace!("ignoring NFS_ACL/ID_MAP/METADATA packet");
                xdr::rpc::prog_unavail_reply_message(xid).serialize(output)?;
                Ok(())
            }
            NFS_LOCALIO_PROGRAM => {
                trace!("Ignoring NFS_LOCALIO packet");
                xdr::rpc::prog_unavail_reply_message(xid).serialize(output)?;
                Ok(())
            }
            _ => {
                warn!("Unknown RPC Program number {} != {}", call.prog, nfs3::PROGRAM);
                xdr::rpc::prog_unavail_reply_message(xid).serialize(output)?;
                Ok(())
            }
        }
        .map(|_| true);

        context.transaction_tracker.mark_processed(xid, &context.client_addr);

        result
    } else {
        error!("Unexpectedly received a Reply instead of a Call");
        io_other("Bad RPC Call format")
    }
}

/// Standard async RPC processing function that can be used with `CommandQueue`
///
/// Processes an RPC command by:
/// 1. Deserializing the RPC message
/// 2. Processing the RPC call according to standard protocol
/// 3. Writing response to output buffer
///
/// # Arguments
///
/// * `data` - Buffer containing RPC message
/// * `output` - Buffer for writing response
/// * `context` - RPC processing context
///
/// # Returns
///
/// `Ok(true)` if response needs to be sent
/// `Ok(false)` if no response needed (e.g. retransmission)
/// `Err` if processing error occurred
pub async fn process_rpc_command(
    data: Vec<u8>,
    output: &mut ResponseBuffer,
    context: &mut Context,
) -> io::Result<bool> {
    // Create cursor for reading data
    let mut input_cursor = Cursor::new(data);

    // Get internal buffer for writing
    let output_buffer = output.get_mut_buffer();
    let mut output_cursor = Cursor::new(output_buffer);

    // Call RPC handler
    let result = handle_rpc(&mut input_cursor, &mut output_cursor, context).await?;

    // If response was generated, return true
    Ok(result)
}
