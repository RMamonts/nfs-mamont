//! NFSv4 (Network File System version 4) protocol implementation as specified in RFC 7530.
//!
//! This module implements the NFS version 4 protocol which introduces several key improvements
//! over NFSv3:
//!
//! - **Compound Operations** - Multiple operations can be combined into a single RPC call
//! - **Stateful Protocol** - Support for file locking, delegation, and client state management
//! - **Strong Security** - Integrated Kerberos v5 support and other security mechanisms
//! - **Internationalization** - UTF-8 support for file names and other text
//! - **Improved Caching** - File delegation allows clients to cache data safely
//! - **Better Error Handling** - More detailed error reporting and recovery mechanisms
//!
//! ## Key Operations
//!
//! The primary operations supported include:
//! - **NULL (0)** - Do nothing (ping the server)
//! - **COMPOUND (1)** - Execute multiple operations atomically
//!
//! The COMPOUND operation is the cornerstone of NFSv4, allowing clients to group
//! multiple file system operations into a single RPC call, reducing network latency
//! and improving consistency guarantees.
//!
//! Each operation handler follows the same pattern as NFSv3, taking RPC parameters
//! and returning appropriate responses via the output stream.

use dashmap::DashMap;
use num_traits::cast::FromPrimitive;
use std::collections::HashMap;
use std::io;
use std::io::{Read, Write};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::warn;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, nfs4, Serialize};
use crate::vfs::NFSv4FileSystem;
use crate::xdr::nfs4::operations::{nfs_argop4, nfs_resop4};
use crate::xdr::nfs4::{
    clientid4, delegation_type, filehandle, nfs_client_id, nfs_fh4, nfs_ftype4, seqid4,
    state_owner_type, state_type, stateid4,
};

mod compound;
mod null;

/// NFS Version 4 constant
pub const VERSION: u32 = 4;

/// Main entry point for handling NFS v4 requests
///
/// This function dispatches incoming RPC calls to the appropriate NFS v4 operation handler.
/// Unlike NFSv3 which supports many individual procedures, NFSv4 primarily uses the
/// COMPOUND operation to execute multiple sub-operations in a single call.
///
/// # Arguments
///
/// * `xid` - Transaction ID from the RPC call
/// * `call` - The RPC call body containing program, version, and procedure numbers  
/// * `input` - Input stream for reading procedure arguments
/// * `output` - Output stream for writing procedure results
/// * `context` - Server context containing the VFS and other state
///
/// # Returns
///
/// * `io::Result<()>` - Ok(()) on success or an error
pub async fn handle_nfs(
    xid: u32,
    call: xdr::rpc::call_body,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> io::Result<()> {
    if call.vers != VERSION {
        warn!("Invalid NFS Version number {} != {}", call.vers, VERSION);
        xdr::rpc::prog_mismatch_reply_message(xid, VERSION).serialize(output)?;
        return Ok(());
    }

    let prog = nfs4::nfs_opnum4::from_u32(call.proc).unwrap_or(nfs4::nfs_opnum4::OP_ILLEGAL);

    match prog {
        nfs4::nfs_opnum4::OP_NULL => null::nfsproc4_null(xid, output)?,
        nfs4::nfs_opnum4::OP_COMPOUND => {
            compound::nfsproc4_compound(xid, input, output, context).await?
        }
        _ => {
            warn!("Unimplemented NFS v4 operation: {:?}", prog);
            xdr::rpc::proc_unavail_reply_message(xid).serialize(output)?;
        }
    }

    Ok(())
}

/// Represents the context for NFSv4 operations.
/// Contains necessary state and configuration for NFSv4 protocol handling.

/// NFSv4.1 operation context (RFC 7530 Section 15.4)
/// Contains the current execution state for NFS operations
pub struct NFSv4Context {
    /// CURRENT filehandle - target of current operation (RFC 7530 Section 15.4.1)
    _current_file_handler: filehandle,
    /// SAVED filehandle - saved for compound operations (RFC 7530 Section 15.4.2)
    _saved_file_handler: filehandle,
    /// CURRENT stateid - for operations requiring state (OPEN, LOCK, etc.)
    _current_stateid: stateid4,
    /// SAVED stateid - saved state for compound operations
    _saved_stateid: stateid4,
    /// NFS minor version being used (0 for v4.0, 1 for v4.1)
    _minor_version: u32,
    /// Server-wide state management
    _nfsv4state: NFSv4State,
}

/// Server-wide NFSv4.0 state (RFC 7530 Section 9)
/// Manages all client-visible server state
pub struct NFSv4State {
    /// Client ID to client details mapping (RFC 7530 Section 9.1.2)
    clients: Arc<DashMap<clientid4, nfs_client_id>>,
    /// Per-client state table
    state_table: Arc<DashMap<clientid4, Vec<State>>>,
    /// Filesystem abstraction layer instances
    fs: Arc<DashMap<String, NFSv4FS>>,
}

/// Represents a single state object on server (RFC 7530 Section 9.1.4)
/// Can be OPEN, LOCK, DELEGATION
pub struct State {
    /// All state entries for this file (organized by clientid)
    states: Arc<DashMap<clientid4, Vec<Arc<state_type>>>>,
    /// Associated filehandle
    filehandle: filehandle,
    /// Type of state (OPEN, LOCK, etc.)
    state_type: state_type,
    /// State owner information
    state_owner: Arc<RwLock<StateOwner>>,
    /// Last used sequence ID (for operation ordering)
    last_sequid: seqid4,
    /// Generation number for state recovery
    generation_number: u32,
}

/// State owner information
/// Identifies the owner of a particular state (open/lock owner)
pub struct StateOwner {
    /// OPEN/LOCK/DELEGATION owner type
    owner_type: state_owner_type,
    /// Opaque owner identifier
    owner_name: Vec<u8>,
    /// Client ID that owns this state
    client: clientid4,
    /// Client information
    client_info: nfs_client_id,
    /// Current sequence ID (for operation ordering)
    seqid: seqid4,
    /// All states owned by this owner (organized by nfs_fh4)
    states: Arc<DashMap<nfs_fh4, Vec<state_type>>>,
    /// Lease expiration time (RFC 7530 Section 9.1.4.4)
    expiration: Duration,
    /// Optional filehandle reference (RFC 5661 Section 2.4.1)
    filehandle: filehandle,
}

/// Filesystem abstraction (RFC 7530 Section 5.1)
/// Represents an exported filesystem with its objects
pub struct NFSv4FS {
    /// Filesystem name/identifier
    fs_name: Vec<u8>,
    /// Exported objects (RFC 7530 Section 5.5)
    exports: Arc<DashMap<nfs_fh4, filehandle>>,
    /// Virtual filesystem implementation (RFC 7530 Section 5.1)
    vfs: Arc<dyn NFSv4FileSystem>,
}
