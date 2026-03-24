//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod allocator;
pub mod consts;
mod context;
mod mount;
mod parser;
mod rpc;
mod serializer;
mod service;
mod task;
pub mod vfs;

use tokio::net::TcpListener;
use tracing::debug;
use tracing_subscriber::EnvFilter;

use crate::task::connection;
use crate::task::global::mount::MountTask;

pub use allocator::Slice;
pub use context::ServerContext;

/// Public export description used to configure MOUNT roots for the server.
pub struct MountExport {
    export: mount::ExportEntry,
    root_handle: vfs::file::Handle,
}

impl MountExport {
    /// Creates an export from already validated NFS path.
    pub fn new(directory: vfs::file::Path, root_handle: vfs::file::Handle) -> Self {
        Self { export: mount::ExportEntry { directory, names: Vec::new() }, root_handle }
    }

    /// Creates an export from a filesystem path string.
    pub fn from_directory_path(
        directory: impl Into<String>,
        root_handle: vfs::file::Handle,
    ) -> std::io::Result<Self> {
        let directory = vfs::file::Path::new(directory.into())?;
        Ok(Self::new(directory, root_handle))
    }
}

/// Initializes tracing logs.
///
/// In debug builds logs are enabled by default. In release builds this is a no-op.
pub fn init_tracing() {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("nfs_mamont=debug"));

    let _ = tracing_subscriber::fmt().with_env_filter(env_filter).try_init();
}

/// Starts the NFS server and processes client connections.
pub async fn handle_forever<V>(
    listener: TcpListener,
    context: ServerContext<V>,
) -> std::io::Result<()>
where
    V: vfs::Vfs + Send + Sync + 'static,
{
    handle_forever_with_exports(listener, context, Vec::new()).await
}

/// Starts the NFS server and processes client connections with explicit MOUNT exports.
pub async fn handle_forever_with_exports<V>(
    listener: TcpListener,
    context: ServerContext<V>,
    exports: Vec<MountExport>,
) -> std::io::Result<()>
where
    V: vfs::Vfs + Send + Sync + 'static,
{
    let export_paths = exports
        .iter()
        .map(|entry| entry.export.directory.as_path().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    debug!(configured_mount_exports = ?export_paths, "server start: configured MOUNT exports");

    let exports = exports
        .into_iter()
        .map(|entry| crate::service::mount::ExportEntryWrapper {
            export: entry.export,
            root_handle: entry.root_handle,
        })
        .collect();

    let (mount_task, mount_sender) = MountTask::new(exports);
    mount_task.spawn();

    loop {
        let (socket, _) = listener.accept().await?;

        socket.set_nodelay(true)?;
        tune_socket_buffers(&socket)?;

        connection::new(socket, mount_sender.clone(), &context).await;
    }
}

#[cfg(target_os = "linux")]
fn tune_socket_buffers(socket: &tokio::net::TcpStream) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

    const SOCKET_BUFFER_SIZE: libc::c_int = 4 * 1024 * 1024;

    let fd = socket.as_raw_fd();
    let value_ptr = &SOCKET_BUFFER_SIZE as *const libc::c_int as *const libc::c_void;
    let value_len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;

    let snd_result = unsafe {
        libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_SNDBUF, value_ptr, value_len)
    };
    if snd_result != 0 {
        return Err(std::io::Error::last_os_error());
    }

    let rcv_result = unsafe {
        libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_RCVBUF, value_ptr, value_len)
    };
    if rcv_result != 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn tune_socket_buffers(_socket: &tokio::net::TcpStream) -> std::io::Result<()> {
    Ok(())
}
