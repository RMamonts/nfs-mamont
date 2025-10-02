//! Implementation of the `UMNT` procedure (procedure 3) for MOUNT version 3 protocol
//! as defined in RFC 1813 section 5.2.3
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.3>.

use std::io;
use std::io::Write;

use tracing::{debug, warn};

use super::{machine_name_from_context, matches_export_path};
use crate::protocol::rpc;
use crate::protocol::xdr::{self, mount, Serialize};
use crate::xdr::mount::dirpath;

/// Handles `MOUNTPROC3_UMNT` procedure.
///
/// Function removes the mount entry from the mount list for
/// the requested diretory.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `args` - Procedure arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing mount signal information
///
/// # Returns
///
/// * `io::Result<()>` - Ok(()) on success or an error
///
/// # Notes
///
/// Clients not always call `umnt` when unmounting a filesystem.
///
/// - Unmounting with 'umount' from 'util-linux' package doesn't call 'umnt'.
///
/// - Unmounting with 'umount.nfs' from 'nfs-utils' package does call 'umnt'.
pub async fn mountproc3_umnt(
    xid: u32,
    args: dirpath,
    output: &mut impl Write,
    context: &rpc::Context,
) -> io::Result<()> {
    let Ok(utf8path) = std::str::from_utf8(&args) else {
        warn!("Invalid UTF-8 path in umnt: {:?}", args);
        return Ok(());
    };
    debug!("mountproc3_umnt({:?},{:?}) ", xid, utf8path);

    if let Some(mount_entry) =
        context.export_table.iter().find(|entry| matches_export_path(utf8path, &entry.export_name))
    {
        if let Some(chan) = &mount_entry.mount_signal {
            let _ = chan.send(false).await;
        }
    }
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    mount::mountstat3::MNT3_OK.serialize(output)?;

    if let Some(machine_name) = machine_name_from_context(context) {
        debug!("client_list: {machine_name} -= {utf8path}");
        context.client_list.entry(machine_name).and_modify(|set| {
            set.remove(&utf8path.to_string());
        });
    }
    Ok(())
}
