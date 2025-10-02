//! Implementation of the `MKDIR` procedure (procedure 9) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.9.
//!
//! The `MKDIR` procedure creates a new directory in the specified parent directory.
//! The client specifies:
//! - The file handle of the parent directory
//! - The name of the new directory
//! - The initial attributes for the new directory
//!
//! On successful return, the server provides:
//! - The file handle of the new directory
//! - The attributes of the new directory
//! - The attributes of the parent directory before and after the operation (weak cache consistency)
//!
//! This procedure fails if the parent directory is read-only, the name already exists,
//! or the user doesn't have appropriate access permissions.

use std::io;
use std::io::{Read, Write};

use tracing::{debug, error, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};
use crate::vfs;

/// Handles `NFSv3` `MKDIR` procedure (procedure 9)
///
/// `MKDIR` creates a new directory.
/// Takes parent directory handle, name for new directory and attributes.
/// Returns file handle and attributes of the newly created directory.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the `MKDIR` arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `io::Result<()>` - Ok(()) on success or an error
pub async fn nfsproc3_mkdir(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> io::Result<()> {
    let args = deserialize::<nfs3::dir::MkDir3Args>(input)?;
    debug!("nfsproc3_mkdir({:?}, {:?}) ", xid, args);

    let fs_id = args.dirops.dir.fs_id;
    let Some(export) = context.export_table.get(&fs_id) else {
        warn!("No export found for fs_id: {}", fs_id);
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        nfs3::NFSStat3::NFS3ErrBadHandle.serialize(output)?;
        nfs3::WCCData::default().serialize(output)?;
        return Ok(());
    };

    // if we do not have write capabilities
    if !matches!(export.vfs.capabilities(), vfs::Capabilities::ReadWrite) {
        warn!("No write capabilities.");
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        nfs3::NFSStat3::NFS3ErrROFS.serialize(output)?;
        nfs3::WCCData::default().serialize(output)?;
        return Ok(());
    }

    // find the directory we are supposed to create the
    // new file in
    let dir_id = export.vfs.fh_to_id(&args.dirops.dir);
    if let Err(stat) = dir_id {
        // directory does not exist
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::WCCData::default().serialize(output)?;
        error!("Directory does not exist");
        return Ok(());
    }
    // found the directory, get the attributes
    let dir_id = dir_id.unwrap();

    // get the object attributes before the write
    let pre_dir_attr = match export.vfs.getattr(dir_id).await {
        Ok(v) => nfs3::PreOpAttr::Some(v.into()),
        Err(stat) => {
            error!("Cannot stat directory");
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            nfs3::WCCData::default().serialize(output)?;
            return Ok(());
        }
    };

    let res = export.vfs.mkdir(dir_id, &args.dirops.name).await;

    // Re-read dir attributes for post op attr
    let post_dir_attr = export.vfs.getattr(dir_id).await.ok();
    let wcc_res = nfs3::WCCData { before: pre_dir_attr, after: post_dir_attr };

    match res {
        Ok((fid, fattr)) => {
            debug!("mkdir success --> {:?}, {:?}", fid, fattr);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::NFSStat3::NFS3Ok.serialize(output)?;
            // serialize CREATE3resok
            let fh = export.vfs.id_to_fh(fid, fs_id);
            nfs3::PostOpFh3::Some(fh).serialize(output)?;
            nfs3::PostOpAttr::Some(fattr).serialize(output)?;
            wcc_res.serialize(output)?;
        }
        Err(e) => {
            debug!("mkdir error {:?} --> {:?}", xid, e);
            // serialize CREATE3resfail
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            e.serialize(output)?;
            wcc_res.serialize(output)?;
        }
    }

    Ok(())
}
