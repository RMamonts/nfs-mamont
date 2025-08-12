//! Implementation of the EXPORT procedure (procedure 5) for `MOUNT` version 3 protocol
//! as defined in RFC 1813 section 5.2.5.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.5>.

use std::io;
use std::io::Write;

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, Serialize};

/// Handles `MOUNTPROC3_EXPORT` procedure.
///
/// Function returns a list of all the exported file
/// systems and which clients are allowed to mount each one.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing export information
///
/// # Returns
///
/// * `io::Result<()>` - Ok(()) on success or an error
pub async fn mountproc3_export(
    xid: u32,
    output: &mut impl Write,
    context: &rpc::Context,
) -> io::Result<()> {
    debug!("mountproc3_export({:?}) ", xid);
    let export_table = context.export_table.read().await;
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    // Serialize each export entry
    for mount_entry in export_table.values() {
        true.serialize(output)?;
        // Dirpath of the export
        mount_entry.export_name.as_bytes().serialize(output)?;
        // No groups
        false.serialize(output)?;
    }
    // No more exports
    false.serialize(output)?;
    Ok(())
}
