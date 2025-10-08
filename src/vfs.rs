//! Virtual File System trait definition.
//!
//! This module defines the `Vfs` trait that must be implemented by any
//! file system that wants to be served via this NFS server.
//!
//! The trait is based on NFS v3 protocol specification (RFC 1813).

use async_trait::async_trait;

/// Virtual File System trait.
///
/// This trait defines the interface that a file system must implement
/// to be used with the NFS server. All methods are async to allow
/// for I/O operations.
#[async_trait]
pub trait Vfs: Sync + Send {
    // TODO: Add methods here as we implement the NFS operations
}

