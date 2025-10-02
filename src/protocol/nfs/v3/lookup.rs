//! Implementation of the `LOOKUP` procedure (procedure 3) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.3.
//!
//! The `LOOKUP` procedure performs file name lookup in a directory. It translates
//! a file name into a file handle that can be used for subsequent operations on the file.
//! The client specifies:
//! - The file handle for the directory to search
//! - The filename to look up within that directory
//!
//! On successful return, the server provides:
//! - The file handle of the requested file
//! - The attributes of the requested file
//! - The attributes of the directory (for cache validation)

use std::io;
use std::io::{Read, Write};

use tracing::{debug, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};

/// Handles `NFSv3` `LOOKUP` procedure (procedure 3)
///
/// `LOOKUP` searches for a file name in a directory and returns the file handle.
/// It takes directory file handle and a file name as input.
/// Returns file handle and attributes of the found file.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the `LOOKUP` arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `io::Result<()>` - Ok(()) on success or an error
pub async fn nfsproc3_lookup(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> io::Result<()> {
    let dir_ops = deserialize::<nfs3::DiropArgs3>(input)?;
    debug!("nfsproc3_lookup({:?},{:?}) ", xid, dir_ops);

    let fs_id = dir_ops.dir.fs_id;
    let Some(export) = context.export_table.get(&fs_id) else {
        warn!("No export found for fs_id: {}", fs_id);
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        nfs3::NFSStat3::NFS3ErrBadHandle.serialize(output)?;
        nfs3::PostOpAttr::None.serialize(output)?;
        return Ok(());
    };

    let dir_id = export.vfs.fh_to_id(&dir_ops.dir);

    // fail if unable to convert file handle
    if let Err(stat) = dir_id {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::PostOpAttr::None.serialize(output)?;
        return Ok(());
    }

    let dirid = dir_id.unwrap();

    let dir_attr = export.vfs.getattr(dirid).await.ok();

    match export.vfs.lookup(dirid, &dir_ops.name).await {
        Ok(fid) => {
            let obj_attr = export.vfs.getattr(fid).await.ok();
            debug!("nfsproc3_lookup success {:?} --> {:?}", xid, obj_attr);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::NFSStat3::NFS3Ok.serialize(output)?;
            export.vfs.id_to_fh(fid, fs_id).serialize(output)?;
            obj_attr.serialize(output)?;
            dir_attr.serialize(output)?;
        }
        Err(stat) => {
            debug!("nfsproc3_lookup error {:?}({:?}) --> {:?}", xid, dir_ops.name, stat);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            dir_attr.serialize(output)?;
        }
    }
    Ok(())
}
