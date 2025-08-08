//! Module for NFSv4 protocol implementation.
//! Provides functionality for working with NFS version 4 protocol context and operations.

use std::io::{Read, Write};

use tracing::error;

use crate::{
    protocol::rpc,
    xdr::{self, rpc::make_success_reply, Serialize},
};

mod operations;

pub const VERSION: u32 = 4;

/// Represents the context for NFSv4 operations.
/// Contains necessary state and configuration for NFSv4 protocol handling.
#[derive(Default)]
pub struct NFSv4Context {
    // TODO: find out what should be here
}

/// Main handler for NFSv4 protocol
///
/// Dispatches NFSv4 RPC calls to appropriate procedure handlers based on procedure number.
/// Validates protocol version and returns appropriate error for unsupported procedures.
/// Acts as the central router for all NFS operations in the server.
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
    _context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    if call.vers != VERSION {
        error!("Invalid NFS Version number {} != {}", call.vers, VERSION);
        xdr::rpc::prog_mismatch_reply_message(xid, VERSION).serialize(output)?;
        return Ok(());
    }

    let msg = make_success_reply(xid);

    let req = operations::Request::deserialize_from_rpc(call, input)?;

    let response = req.execute()?;

    msg.serialize(output)?;
    response.serialize_no_resop(output)?;

    Ok(())
}
