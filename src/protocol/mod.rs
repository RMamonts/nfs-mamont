//! Protocol module implements the NFS version 3 protocol suite as specified in RFC 1813.
//!
//! This module contains three main components:
//!
//! - `xdr`: External Data Representation (XDR) for serialization and deserialization
//!   of data structures according to RFC 1832.
//!
//! - `nfs`: Implementation of the NFS version 3 protocol operations, including the main
//!   NFSv3 protocol (RFC 1813), the MOUNT protocol, and the PORTMAP protocol.
//!
//! - `rpc`: Remote Procedure Call (RPC) protocol implementation for handling client
//!   requests, transaction tracking, and server context management as defined in RFC 1057.
//!
//! The NFS protocol is a network file system protocol originally designed by Sun Microsystems.
//! It is stateless, using file handles to identify files rather than path names, and supports
//! strong cache coherency and Kerberos authentication.

pub mod nfs;
pub mod rpc;
pub mod xdr;
