//! Implementation of the `GETATTR` procedure (procedure 1) for NFS version 3 protocol
//! as defined in RFC 1813 section 3.3.1.
//!
//! The `GETATTR` procedure retrieves file attributes for a specified file system object.
//! It is used by NFS clients to:
//! - Check if cached attributes are still valid
//! - Get initial attributes for files and directories
//! - Check file/directory sizes, permissions, ownership, etc.
//!
//! `GETATTR` takes a file handle as input and returns the complete file attribute
//! structure defined in RFC 1813 section 2.3.5 (fattr3).

use std::io::{Read, Write};

use tracing::{debug, error};

use crate::protocol::rpc;
use crate::protocol::xdr::{self, deserialize, nfs3, Serialize};

/// Handles `NFSv3` `GETATTR` procedure (procedure 1)
///
/// `GETATTR` retrieves attributes for a specified file system object.
/// Takes a file handle as input and returns the file's attributes.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `input` - Input stream containing the file handle
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing VFS
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub async fn nfsproc3_getattr(
    xid: u32,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let handle = deserialize::<nfs3::nfs_fh3>(input)?;
    debug!("nfsproc3_getattr({:?},{:?}) ", xid, handle);

    let id = context.vfs.fh_to_id(&handle);
    // fail if unable to convert file handle
    if let Err(stat) = id {
        xdr::rpc::make_success_reply(xid).serialize(output)?;
        stat.serialize(output)?;
        return Ok(());
    }
    let id = id.unwrap();
    match context.vfs.getattr(id).await {
        Ok(fh) => {
            debug!(" {:?} --> {:?}", xid, fh);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            nfs3::nfsstat3::NFS3_OK.serialize(output)?;
            fh.serialize(output)?;
        }
        Err(stat) => {
            error!("nfsproc3_getattr error {:?} --> {:?}", xid, stat);
            xdr::rpc::make_success_reply(xid).serialize(output)?;
            stat.serialize(output)?;
        }
    }
    Ok(())
}
