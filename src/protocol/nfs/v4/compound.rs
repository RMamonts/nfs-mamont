//! Implementation of the COMPOUND procedure (procedure 1) for NFS version 4 protocol
//! as defined in RFC 7530 section 15.2.
//!
//! The COMPOUND procedure is the primary mechanism in NFSv4 for executing multiple
//! operations in a single RPC call. This provides several benefits:
//! - Reduced network latency by batching operations
//! - Atomic execution - all operations succeed or all fail
//! - Improved consistency and caching behavior
//!
//! Each COMPOUND request contains an array of individual operations, and the
//! server executes them sequentially, stopping on the first error.

use std::io;
use std::io::{Read, Write};

use tracing::debug;

use crate::protocol::nfs::v4::NFSv4Context;
use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs4, Serialize};

/// Handles NFSv4 COMPOUND procedure (procedure 1)
///
/// COMPOUND executes multiple operations atomically in a single RPC call.
/// The procedure reads the compound arguments, executes each operation
/// in sequence, and returns the results.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the compound arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS and state
///
/// # Returns
///
/// * `io::Result<()>` - Ok(()) on success or an error
pub async fn nfsproc4_compound(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    _context: &rpc::Context,
    _compound_context: NFSv4Context,
) -> io::Result<()> {
    debug!("nfsproc4_compound({:?})", xid);

    // Deserialize compound arguments
    let args: nfs4::COMPOUND4args = deserialize(input)?;
    debug!(
        "COMPOUND args: tag={}, minorversion={}, {} operations",
        args.tag,
        args.minorversion,
        args.argarray.len()
    );

    let response = nfs4::COMPOUND4res {
        status: nfs4::nfsstat4::NFS4_OK,
        tag: args.tag.clone(),
        resarray: Vec::new(),
    };

    // TODO: Execute operations from argarray
    // For now, just return success with empty result array

    // Send RPC success reply followed by compound response
    let msg = xdr::rpc::make_success_reply(xid);
    msg.serialize(output)?;

    debug!("COMPOUND response: status={:?}, {} results", response.status, response.resarray.len());
    response.serialize(output)?;

    Ok(())
}
