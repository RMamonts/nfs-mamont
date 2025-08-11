//! Implementation of the NULL procedure (procedure 0) for NFS version 4 protocol
//! as defined in RFC 7530 section 15.1.
//!
//! The NULL procedure is identical to NFSv3 NULL - it does no work and is typically used to:
//! - Check if the server is responding (ping)
//! - Measure basic RPC round-trip time
//! - Validate RPC credentials
//!
//! NULL takes no arguments and returns no results, just an RPC response indicating success.

use std::io::Write;

use tracing::debug;

use crate::protocol::xdr::{self, Serialize};

/// Handles NFSv4 NULL procedure (procedure 0)
///
/// NULL is a no-operation RPC call used to check if the server is responding.
/// Takes no arguments and returns nothing but an RPC success.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `output` - Output stream for writing the response
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub fn nfsproc4_null(xid: u32, output: &mut impl Write) -> Result<(), anyhow::Error> {
    debug!("nfsproc4_null({:?})", xid);
    let msg = xdr::rpc::make_success_reply(xid);
    debug!("\t{:?} --> {:?}", xid, msg);
    msg.serialize(output)?;
    Ok(())
}
