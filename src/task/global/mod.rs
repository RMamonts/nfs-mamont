//! Global task management for the NFS server.
//!
//! This module provides tasks that exists across all NFS client connections
//!
//! Planed global tasks:
//! - MOUNT (TODO: <https://github.com/RMamonts/nfs-mamont/issues/115>))
//! - NLM (TODO: TBD)
//! - AUTH (TODO: TBD)

pub mod mount;
