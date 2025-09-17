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
use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;
use std::{io, net::IpAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, trace};

use crate::protocol::nfs::portmap::PortmapTable;
use crate::protocol::rpc::Context;
use crate::protocol::{rpc, xdr};
use crate::utils::error::io_other;
use crate::vfs::NFSFileSystem;

/// Default transaction retention period
const TRANSACTION_RETENTION_PERIOD: Duration = Duration::from_secs(60);

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
pub type NFSExportTable = DashMap<crate::xdr::nfs3::fs_id, NFSExportTableEntry>;

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
    pub data: Vec<u8>,
    /// Context associated with this command
    pub context: Context,
}

impl RpcCommand {
    pub async fn read_command_from_socket(
        &mut self,
        socket: &mut ReadHalf<TcpStream>,
    ) -> io::Result<()> {
        let mut is_last = false;
        let mut header_buf = [0_u8; 4];
        while !is_last {
            socket.read_exact(&mut header_buf).await?;
            let fragment_header = u32::from_be_bytes(header_buf);
            is_last = (fragment_header & (1 << 31)) > 0;
            let length = (fragment_header & ((1 << 31) - 1)) as usize;
            trace!("Reading fragment length:{}, last:{}", length, is_last);
            let start_offset = self.data.len();
            self.data.resize(self.data.len() + length, 0);
            socket.read_exact(&mut self.data[start_offset..]).await?;
            trace!("Finishing Reading fragment length:{}, last:{}", length, is_last);
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
        const MAX_FRAGMENT_SIZE: usize = (1 << 31) - 1;

        let mut offset = 0;
        while offset < self.buffer.len() {
            // Calculate the size of this fragment
            let remaining = self.buffer.len() - offset;
            let fragment_size = std::cmp::min(remaining, MAX_FRAGMENT_SIZE);

            // Determine if this is the last fragment
            let is_last = offset + fragment_size >= self.buffer.len();

            // Create the fragment header
            // The highest bit indicates if this is the last fragment
            let fragment_header =
                if is_last { fragment_size as u32 + (1 << 31) } else { fragment_size as u32 };

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
    let (message_handler, mut msgrecvchan) = rpc::SocketMessageHandler::new();

    let (mut readhalf, mut writehalf) = tokio::io::split(socket);

    tokio::spawn(async move {
        loop {
            let mut command = RpcCommand { data: Vec::new(), context: context.clone() };
            match command.read_command_from_socket(&mut readhalf).await {
                Ok(()) => {
                    //here some processing - actually sending to processing rpc task
                    if !message_handler.command_queue.submit_command(command) {
                        error!("Failed to submit command to queue");
                        return io_other::<(), &str>("Command queue error");
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    if command.data.is_empty() {
                        trace!("Connection closed before receiving any data");
                    } else {
                        error!("Connection closed during command transmission");
                        return io_other("Early socket closing");
                    }
                }
                Err(e) => {
                    error!("Message loop broken due to {:?}", e);
                    return Err(e);
                }
            }
        }
    });

    tokio::spawn(async move {
        while let Some(result) = msgrecvchan.recv().await {
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
