//! Implementation of the `LINK` procedure (procedure 15) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.15.
//!
//! The `LINK` procedure creates a hard link from one file to another. A hard link
//! is a second directory entry referring to the same file with an identical
//! file system object.
//!
//! The client specifies:
//! - The file handle for the existing file (target)
//! - The directory file handle and name for the new link (where to create the link)
//!
//! On successful return, the server provides:
//! - The file attributes of the target file after the operation
//! - The attributes of the directory before and after the operation (weak cache consistency)
//!
//! Hard links can be created only within a single file system (volume).
//! Servers should return `NFS3ERR_XDEV` if a cross-device link is attempted.

use std::io;
use std::io::{Read, Write};

use tracing::{debug, warn};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};
use crate::vfs;

/// Handles `NFSv3` `LINK` procedure (procedure 15)
///
/// `LINK` creates a hard link to an existing file.
/// Takes file handle for target file, directory handle, and name for the new link.
/// Returns file attributes and directory attributes before and after the operation.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the `LINK` arguments
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `io::Result<()>` - Ok(()) on success or an error
pub async fn nfsproc3_link(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> io::Result<()> {
    let args = deserialize::<nfs3::file::LINK3args>(input)?;
    debug!("nfsproc3_link({:?}, {:?}) ", xid, args);

    let file_fs_id = args.file.fs_id;
    let link_fs_id = args.link.dir.fs_id;

    if file_fs_id != link_fs_id {
        warn!("Trying to hard link across different file systems");
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        nfs3::NFSStat3::NFS3ErrXdev.serialize(output)?;
        nfs3::WCCData::default().serialize(output)?;
        return Ok(());
    }

    let Some(export) = context.export_table.get(&file_fs_id) else {
        warn!("No export found for fs_id: {}", file_fs_id);
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        nfs3::NFSStat3::NFS3ErrBadHandle.serialize(output)?;
        nfs3::PostOpAttr::None.serialize(output)?;
        nfs3::WCCData::default().serialize(output)?;
        return Ok(());
    };

    // if we do not have write capabilities
    if !matches!(export.vfs.capabilities(), vfs::Capabilities::ReadWrite) {
        warn!("No write capabilities.");
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        nfs3::NFSStat3::NFS3ErrROFS.serialize(output)?;
        nfs3::PostOpAttr::None.serialize(output)?;
        nfs3::WCCData::default().serialize(output)?;
        return Ok(());
    }

    // Get the file id
    let file_id = export.vfs.fh_to_id(&args.file);
    if let Err(stat) = file_id {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::PostOpAttr::None.serialize(output)?;
        nfs3::WCCData::default().serialize(output)?;
        return Ok(());
    }
    let file_id = file_id.unwrap();

    // Get the directory id
    let dir_id = export.vfs.fh_to_id(&args.link.dir);
    if let Err(stat) = dir_id {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        nfs3::PostOpAttr::None.serialize(output)?;
        nfs3::WCCData::default().serialize(output)?;
        return Ok(());
    }
    let dir_id = dir_id.unwrap();

    // Get the directory attributes before the operation
    let pre_dir_attr = export.vfs.getattr(dir_id).await.map(nfs3::WCCAttr::from).ok();

    // Call VFS link method
    match export.vfs.link(file_id, dir_id, &args.link.name).await {
        Ok(fattr) => {
            // Get file attributes
            let file_attr = nfs3::PostOpAttr::Some(fattr);

            // Get the directory attributes after the operation
            let post_dir_attr = export.vfs.getattr(dir_id).await.ok();

            let wcc_res = nfs3::WCCData { before: pre_dir_attr, after: post_dir_attr };

            debug!("nfsproc3_link success");
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::NFSStat3::NFS3Ok.serialize(output)?;
            file_attr.serialize(output)?;
            wcc_res.serialize(output)?;
        }
        Err(stat) => {
            // Get file attributes
            let file_attr = export.vfs.getattr(file_id).await.ok();

            // Get the directory attributes after the operation (unchanged)
            let post_dir_attr = export.vfs.getattr(dir_id).await.ok();

            let wcc_res = nfs3::WCCData { before: pre_dir_attr, after: post_dir_attr };

            debug!("nfsproc3_link failed: {:?}", stat);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
            file_attr.serialize(output)?;
            wcc_res.serialize(output)?;
        }
    }

    Ok(())
}
