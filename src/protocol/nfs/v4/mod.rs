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

use std::io::{Read, Write};

use num_traits::cast::FromPrimitive;
use tracing::warn;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, nfs4, Serialize};

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
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn handle_nfs(
    xid: u32,
    call: xdr::rpc::call_body,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
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
#[derive(Default)]
pub struct NFSv4Context {
    // TODO: find out what should be here
}
