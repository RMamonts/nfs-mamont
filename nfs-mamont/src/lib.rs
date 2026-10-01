//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod allocator;
mod backend;
pub mod consts;
pub mod mount;
#[allow(dead_code)]
mod nlm;
mod parser;
mod rpc;
mod rpcbind;
mod serializer;
mod server;
pub mod service;
mod task;
pub mod vfs;

use tracing_subscriber::EnvFilter;

pub use allocator::{Allocator, Buffer, Impl, Slice, UnownedBuffer};
pub use backend::BackendRegistry;
pub use rpc::auth;
pub use server::{Server, ServerBuilder};
pub use vfs::file::BackendId;

/// Initializes tracing logs.
///
/// In debug builds logs are enabled by default. In release builds this is a no-op.
pub fn init_tracing() {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("nfs_mamont=debug"));

    let _ = tracing_subscriber::fmt().with_env_filter(env_filter).try_init();
}
