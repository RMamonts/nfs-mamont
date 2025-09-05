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
use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;
use std::{io, net::IpAddr};

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

use crate::protocol::nfs::portmap::PortmapTable;
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
}

/// Generates a local loopback IP address from a 16-bit host number
/// Used for creating multiple local test addresses in the 127.88.x.y range
pub fn generate_host_ip(hostnum: u16) -> String {
    format!("127.88.{}.{}", ((hostnum >> 8) & 0xFF) as u8, (hostnum & 0xFF) as u8)
}

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
async fn process_socket(
    mut socket: tokio::net::TcpStream,
    context: rpc::Context,
) -> io::Result<()> {
    let (mut message_handler, mut socksend, msgrecvchan) = rpc::SocketMessageHandler::new(&context);
    let _ = socket.set_nodelay(true);

    tokio::spawn(async move {
        loop {
            if let Err(e) = message_handler.read().await {
                debug!("Message loop broken due to {:?}", e);
                break;
            }
        }
    });
    loop {
        tokio::select! {
            _ = socket.readable() => {
                let mut buf = [0; 128_000];

                match socket.try_read(&mut buf) {
                    Ok(0) => {
                        return Ok(());
                    }
                    Ok(n) => {
                        let _ = socksend.write_all(&buf[..n]).await;
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // do nothing
                    }
                    Err(e) => {
                        debug!("Message handling closed : {:?}", e);
                        return Err(e);
                    }
                }

            },
            reply = msgrecvchan.recv() => {
                match reply {
                    Ok(Err(e)) => {
                        debug!("Message handling closed : {:?}", e);
                        return Err(e);
                    }
                    Ok(Ok(msg)) => {
                        if let Err(e) = rpc::write_fragment(&mut socket, &msg).await {
                            error!("Write error {:?}", e);
                        }
                    }
                    _ => {
                        return io_other("Unexpected socket context termination");
                    }
                }
            }
        }
    }
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
            };
            info!("Accepting connection from {}", context.client_addr);
            debug!("Accepting socket {:?} {:?}", socket, context);
            tokio::spawn(async move {
                let _ = process_socket(socket, context).await;
            });
        }
    }
}
