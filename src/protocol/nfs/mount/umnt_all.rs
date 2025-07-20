//! Implementation of the UMNTALL procedure (procedure 4) for MOUNT version 3 protocol
//! as defined in RFC 1813 section 5.2.4
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.4>.

use std::io;
use std::io::Write;

use tracing::{debug, error};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, mount, Serialize};

/// Handles `MOUNTPROC3_UMNTALL` procedure.
///
/// Function removes all of the mount entries for
/// this client previously recorded by calls to MNT.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing mount signal information
///
/// # Returns
///
/// * `io::Result<()>` - Ok(()) on success or an error
pub async fn mountproc3_umnt_all(
    xid: u32,
    output: &mut impl Write,
    context: &rpc::Context,
) -> io::Result<()> {
    debug!("mountproc3_umnt_all({:?}) ", xid);

    for mount_entry in context.export_table.read().await.values() {
        // Notify the mount signal channel if it exists
        if let Some(ref chan) = mount_entry.mount_signal {
            let _ = chan.send(false).await;
        }
    }
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    mount::mountstat3::MNT3_OK.serialize(output)?;

    if let Ok(machine_name) = String::from_utf8(context.auth.machinename.clone()) {
        context.client_list.write().await.entry(machine_name).and_modify(|set| set.clear());
    } else {
        error!("Failed to convert machine name to UTF-8");
    }

    Ok(())
}
