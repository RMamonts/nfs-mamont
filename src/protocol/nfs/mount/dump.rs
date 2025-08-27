//! Implementation of the `DUMP` procedure (procedure 2) for MOUNT version 3 protocol
//! as defined in RFC 1813 section 5.2.2
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.2>.
//!

use std::io;
use std::io::{Read, Write};
use tracing::debug;

use crate::protocol::rpc::Context;
use crate::xdr;
use crate::xdr::Serialize;

/// Handles `MOUNTPROC3_DUMP` procedure.
///
/// This procedure returns a list of all the remote filesystems that are
/// currently mounted by clients. Each entry includes the hostname of the
/// client and the directory path that is mounted.
///
/// # Arguments
/// * `xid` - RPC transaction identifier
/// * `_input` - Input stream (unused for this procedure)
/// * `output` - Output stream for writing the response
/// * `context` - RPC context containing client mount information
///
/// # Returns
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
///
pub async fn mountproc3_dump(
    xid: u32,
    _input: &mut impl Read,
    output: &mut impl Write,
    context: &Context,
) -> io::Result<()> {
    debug!("mountproc3_dump({:?}) ", xid);
    debug!("client list: {:?}", context.client_list);

    xdr::rpc::make_success_reply(xid).serialize(output)?;
    context.client_list.iter().try_for_each(|client_entry| {
        client_entry.iter().try_for_each(|path| {
            true.serialize(output)?;
            client_entry.key().serialize(output)?;
            path.serialize(output)?;
            io::Result::Ok(())
        })
    })?;
    false.serialize(output)?;
    Ok(())
}
