//! Implementation of the `UMNT` procedure (procedure 3) for MOUNT version 3 protocol
//! as defined in RFC 1813 section 5.2.3
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.3>.

use std::io;
use std::io::{Read, Write};

use tracing::{debug, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, mount, Serialize};

/// Handles `MOUNTPROC3_UMNT` procedure.
///
/// Function removes the mount entry from the mount list for
/// the requested diretory.
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
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> io::Result<()> {
    let path = deserialize::<Vec<_>>(input)?;
    let Ok(utf8path) = std::str::from_utf8(&path) else {
        warn!("Invalid UTF-8 path in umnt: {:?}", path);
        return Ok(());
    };
    debug!("mountproc3_umnt({:?},{:?}) ", xid, utf8path);

    let export_table = context.export_table.read().await;
    if let Some(mount_entry) =
        export_table.values().find(|entry| utf8path.starts_with(&entry.export_name))
    {
        if let Some(ref chan) = mount_entry.mount_signal {
            let _ = chan.send(false).await;
        }
    }
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    mount::mountstat3::MNT3_OK.serialize(output)?;
    Ok(())
}
