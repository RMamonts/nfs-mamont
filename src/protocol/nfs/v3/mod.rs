//! `NFSv3` (Network File System version 3) protocol implementation as specified in RFC 1813.
//!
//! This module implements all 21 procedure calls defined in the NFS version 3 protocol:
//!
//! 1. `NULL` - Do nothing (ping the server)
//! 2. `GETATTR` - Get file attributes
//! 3. `SETATTR` - Set file attributes
//! 4. `LOOKUP` - Look up file name
//! 5. `ACCESS` - Check access permission
//! 6. `READLINK` - Read from symbolic link
//! 7. `READ` - Read from file
//! 8. `WRITE` - Write to file
//! 9. `CREATE` - Create a file
//! 10. `MKDIR` - Create a directory
//! 11. `SYMLINK` - Create a symbolic link
//! 12. `MKNOD` - Create a special device
//! 13. `REMOVE` - Remove a file
//! 14. `RMDIR` - Remove a directory
//! 15. `RENAME` - Rename a file or directory
//! 16. `LINK` - Create a hard link
//! 17. `READDIR` - Read from directory
//! 18. `READDIRPLUS` - Extended read from directory
//! 19. `FSSTAT` - Get file system statistics
//! 20. `FSINFO` - Get file system information
//! 21. `PATHCONF` - Get path configuration
//! 22. `COMMIT` - Commit cached data
//!
//! Each procedure is implemented in its own module and registered with the main
//! dispatcher function (`handle_nfs`). The dispatcher validates the protocol version
//! and routes incoming RPC requests to the appropriate handler based on the procedure number.
//!
//! `NFSv3` offers several improvements over `NFSv2`, including:
//! - Support for files larger than 2GB
//! - Safe asynchronous writes with the `COMMIT` operation
//! - More robust error reporting with detailed status codes
//! - Support for 64-bit file sizes and offsets
//! - Better attribute caching with the `ACCESS` procedure
//! - Enhanced directory reading with `READDIRPLUS`

use std::io;
use std::io::{Read, Write};

use num_traits::cast::FromPrimitive;
use tokio::sync::RwLockReadGuard;
use tracing::warn;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, nfs3, Serialize};

mod access;
mod commit;
mod create;
mod fsinfo;
mod fsstat;
mod getattr;
mod link;
mod lookup;
mod mkdir;
mod mknod;
mod null;
mod pathconf;
mod read;
mod readdir;
mod readdirplus;
mod readlink;
mod remove;
mod rename;
mod setattr;
mod symlink;
mod write;

use crate::tcp::{NFSExportTable, NFSExportTableEntry};
use crate::xdr::rpc::make_success_reply;
use crate::xdr::Deserialize;
use access::nfsproc3_access;
use commit::nfsproc3_commit;
use create::nfsproc3_create;
use fsinfo::nfsproc3_fsinfo;
use fsstat::nfsproc3_fsstat;
use getattr::nfsproc3_getattr;
use link::nfsproc3_link;
use lookup::nfsproc3_lookup;
use mkdir::nfsproc3_mkdir;
use mknod::nfsproc3_mknod;
use null::nfsproc3_null;
use pathconf::nfsproc3_pathconf;
use read::nfsproc3_read;
use readdir::nfsproc3_readdir;
use readdirplus::nfsproc3_readdirplus;
use readlink::nfsproc3_readlink;
use remove::nfsproc3_remove;
use rename::nfsproc3_rename;
use setattr::nfsproc3_setattr;
use symlink::nfsproc3_symlink;
use write::nfsproc3_write;

/// Main handler for `NFSv3` protocol
///
/// Dispatches `NFSv3` RPC calls to appropriate procedure handlers based on procedure number.
/// Validates protocol version and returns appropriate error for unsupported procedures.
/// Acts as the central router for all NFS operations in the server.
///
/// # Arguments
///
/// * `xid` - Transaction ID from the RPC call
/// * `call` - The RPC call body containing program, version, and procedure numbers
/// * `input` - Input stream for reading procedure arguments
/// * `output` - Output stream for writing procedure results
/// * `context` - Server context containing the VFS and other state
///
/// # Returns
///
/// * `io::Result<()>` - Ok(()) on success or an error
pub async fn handle_nfs(
    xid: u32,
    call: xdr::rpc::call_body,
    input: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> io::Result<()> {
    if call.vers != nfs3::VERSION {
        warn!("Invalid NFS Version number {} != {}", call.vers, nfs3::VERSION);
        // TODO: Use prog_version_range_mismatch_reply_message with proper version range
        // Currently this only reports NFS v3 support, but server actually supports v3-v4
        xdr::rpc::prog_mismatch_reply_message(xid, nfs3::VERSION).serialize(output)?;
        return Ok(());
    }
    let prog = nfs3::NFSProgram::from_u32(call.proc).unwrap_or(nfs3::NFSProgram::INVALID);

    match prog {
        nfs3::NFSProgram::NFSPROC3_NULL => nfsproc3_null(xid, output)?,
        nfs3::NFSProgram::NFSPROC3_GETATTR => nfsproc3_getattr(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_LOOKUP => nfsproc3_lookup(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_READ => nfsproc3_read(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_FSINFO => nfsproc3_fsinfo(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_ACCESS => nfsproc3_access(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_PATHCONF => {
            nfsproc3_pathconf(xid, input, output, context).await?;
        }
        nfs3::NFSProgram::NFSPROC3_FSSTAT => nfsproc3_fsstat(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_READDIR => nfsproc3_readdir(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_READDIRPLUS => {
            nfsproc3_readdirplus(xid, input, output, context).await?;
        }
        nfs3::NFSProgram::NFSPROC3_WRITE => nfsproc3_write(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_CREATE => nfsproc3_create(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_SETATTR => nfsproc3_setattr(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_REMOVE => nfsproc3_remove(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_RMDIR => nfsproc3_remove(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_RENAME => nfsproc3_rename(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_MKDIR => nfsproc3_mkdir(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_SYMLINK => nfsproc3_symlink(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_READLINK => {
            nfsproc3_readlink(xid, input, output, context).await?;
        }
        nfs3::NFSProgram::NFSPROC3_MKNOD => nfsproc3_mknod(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_LINK => nfsproc3_link(xid, input, output, context).await?,
        nfs3::NFSProgram::NFSPROC3_COMMIT => nfsproc3_commit(xid, input, output, context).await?,
        _ => {
            warn!("Unimplemented message {:?}", prog);
            xdr::rpc::proc_unavail_reply_message(xid).serialize(output)?;
        }
    }
    Ok(())
}

#[allow(dead_code)]
trait NfsProc {
    type Args: Deserialize;
    type ResOk: Serialize;
    type ResFail: Serialize + Default;

    async fn handle(
        xid: u32,
        input: &mut impl Read,
        output: &mut impl Write,
        context: &rpc::Context,
    ) -> io::Result<()>;

    /// Retrieve the export entry for the given filesystem ID.
    /// - If the export is found, returns it.
    /// - If not found, construct and send an error reply with [`nfs3::nfsstat3::NFS3ERR_BADHANDLE`] status code and default [`Self::ResFail`]
    async fn get_export_or_reply<'a>(
        xid: u32,
        output: &mut impl Write,
        guard: &'a RwLockReadGuard<'_, NFSExportTable>,
        fs_id: nfs3::fs_id,
    ) -> io::Result<Option<&'a NFSExportTableEntry>> {
        let Some(export) = guard.get(&fs_id) else {
            warn!("No export found for fs_id: {}", fs_id);
            Self::error_reply_default(xid, output, nfs3::nfsstat3::NFS3ERR_BADHANDLE)?;
            return Ok(None);
        };
        Ok(Some(export))
    }

    /// Send a successful reply with the given transaction ID and data.
    fn success_reply(xid: u32, output: &mut impl Write, data: Self::ResOk) -> io::Result<()> {
        make_success_reply(xid).serialize(output)?;
        nfs3::nfsstat3::NFS3_OK.serialize(output)?;
        data.serialize(output)?;
        Ok(())
    }

    /// Send an error reply with the given transaction ID, status code and error reply data.
    fn error_reply(
        xid: u32,
        output: &mut impl Write,
        status_code: nfs3::nfsstat3,
        data: Self::ResFail,
    ) -> io::Result<()> {
        make_success_reply(xid).serialize(output)?;
        status_code.serialize(output)?;
        data.serialize(output)?;
        Ok(())
    }

    /// Send an error reply with the given transaction ID, status code and default error reply data.
    fn error_reply_default(
        xid: u32,
        output: &mut impl Write,
        status_code: nfs3::nfsstat3,
    ) -> io::Result<()> {
        Self::error_reply(xid, output, status_code, Self::ResFail::default())
    }
}

/// Macro to check if the export has write capabilities.
///
/// If the export does not have write capabilities, it sends an error reply with [`nfs3::nfsstat3::NFS3ERR_ROFS`] status code and returns early.
#[macro_export]
macro_rules! check_write_capabilities_or_return {
    ($export:expr, $xid:expr, $output:expr) => {
        if !matches!($export.vfs.capabilities(), vfs::Capabilities::ReadWrite) {
            warn!("No write capabilities.");
            Self::error_reply_default($xid, $output, nfs3::nfsstat3::NFS3ERR_ROFS)?;
            return Ok(());
        }
    };
}
