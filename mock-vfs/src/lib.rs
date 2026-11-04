//! Mock [`Vfs`](nfs_mamont::vfs::Vfs) backend and benchmark tooling for NFS Mamont.
//!
//! The crate is a workspace member that is never published. It exists to measure the
//! server itself (RPC parsing, buffer allocation, serialization, TCP handling) against
//! a backend whose behaviour is fully under control:
//!
//! - [`mock::MockVfs`] serves an infinite, deterministic directory tree from memory and
//!   answers every procedure after a configurable delay, see [`config::Latency`];
//! - [`server`] wires the mock into [`nfs_mamont::handle_forever`] exactly like a real
//!   deployment would;
//! - [`client`] is a minimal NFSv3/MOUNT client used by the `nfs-bench` load generator
//!   and by the integration tests.
//!
//! Binaries: `mock-nfs-server` (standalone server) and `nfs-bench` (load generator).
//! See the crate `README.md` for usage.

pub mod cli;
pub mod client;
pub mod config;
pub mod hist;
pub mod latency;
pub mod mock;
pub mod server;
pub mod tree;
