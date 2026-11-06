//! NFS Mamont - A Network File System (NFS) server implementation in Rust.

mod allocator;
mod backend;
pub mod connection_id;
pub mod consts;
pub mod mount;
pub mod nlm;
mod parser;
mod rpc;
mod rpcbind;
mod serializer;
mod server;
mod task;
pub mod vfs;

pub use allocator::{Allocator, Buffer};
pub use backend::BackendRegistry;
pub use rpc::auth;
pub use rpc::AuthFlavor;
pub use server::{Server, ServerCommand, ServerConfig, ServerContext, ServerHandle, ServerState};
pub use vfs::file::BackendId;
