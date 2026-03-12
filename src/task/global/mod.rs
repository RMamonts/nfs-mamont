//! Global task management for the NFS server.
//!
//! This module provides tasks that exist across all NFS client connections
//!
//! Planned global tasks:
//! - MOUNT (TODO: <https://github.com/RMamonts/nfs-mamont/issues/115>)
//! - NLM (TODO: TBD)
//! - AUTH (TODO: TBD)

pub mod mount;
