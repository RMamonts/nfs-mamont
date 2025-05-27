//! Implementation of the UMNT procedure (procedure 3) for MOUNT version 3 protocol
//! as defined in RFC 1813 Appendix I section I.4.3.
//!
//! The UMNT procedure removes a mount point that was previously established with the MNT procedure.
//! If the client is finished with a file system, it should use this procedure to notify the server.
//! This procedure is typically used during client shutdown or when a file system is unmounted.
//!
//! UMNT takes a directory path and returns nothing.

use std::io::{Read, Write};

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, mount, XDR};

/// Handles MOUNT protocol UMNT procedure (procedure 3)
///
/// UMNT removes a mount for the specified path.
/// Takes a directory path to unmount from the client.
/// Sends unmount notification signal if configured.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the directory path to unmount
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing mount signal information
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn mountproc3_umnt(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let mut path = mount::dirpath::new();
    path.deserialize(input)?;
    let utf8path = std::str::from_utf8(&path).unwrap_or_default();
    debug!("mountproc3_umnt({:?},{:?}) ", xid, utf8path);
    if let Some(ref chan) = context.mount_signal {
        let _ = chan.send(false).await;
    }
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    mount::mountstat3::MNT3_OK.serialize(output)?;
    Ok(())
}
