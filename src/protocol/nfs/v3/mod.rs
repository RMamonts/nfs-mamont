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
use std::io::Write;

use tracing::warn;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, Serialize};

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

use crate::xdr::nfs3::NFSv3_args;
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
    arg: io::Result<Box<NFSv3_args>>,
    output: &mut impl Write,
    context: &rpc::Context,
) -> io::Result<()> {
    let arg = arg?;
    match *arg {
        NFSv3_args::NULL => nfsproc3_null(xid, output),
        NFSv3_args::GETATTR(proc_args) => nfsproc3_getattr(xid, proc_args, output, context).await,
        NFSv3_args::SETATTR(proc_args) => nfsproc3_setattr(xid, proc_args, output, context).await,
        NFSv3_args::LOOKUP(proc_args) => nfsproc3_lookup(xid, proc_args, output, context).await,
        NFSv3_args::ACCESS(proc_args) => nfsproc3_access(xid, proc_args, output, context).await,
        NFSv3_args::READLINK(proc_args) => nfsproc3_readlink(xid, proc_args, output, context).await,
        NFSv3_args::READ(proc_args) => nfsproc3_read(xid, proc_args, output, context).await,
        NFSv3_args::WRITE(proc_args) => nfsproc3_write(xid, proc_args, output, context).await,
        NFSv3_args::CREATE(proc_args) => nfsproc3_create(xid, proc_args, output, context).await,
        NFSv3_args::MKDIR(proc_args) => nfsproc3_mkdir(xid, proc_args, output, context).await,
        NFSv3_args::SYMLINK(proc_args) => nfsproc3_symlink(xid, proc_args, output, context).await,
        NFSv3_args::MKNOD(proc_args) => nfsproc3_mknod(xid, proc_args, output, context).await,
        NFSv3_args::REMOVE(proc_args) => nfsproc3_remove(xid, proc_args, output, context).await,
        NFSv3_args::RMDIR(proc_args) => nfsproc3_remove(xid, proc_args, output, context).await,
        NFSv3_args::RENAME(proc_args) => nfsproc3_rename(xid, proc_args, output, context).await,
        NFSv3_args::LINK(proc_args) => nfsproc3_link(xid, proc_args, output, context).await,
        NFSv3_args::READDIR(proc_args) => nfsproc3_readdir(xid, proc_args, output, context).await,
        NFSv3_args::READDIRPLUS(proc_args) => {
            nfsproc3_readdirplus(xid, proc_args, output, context).await
        }
        NFSv3_args::FSSTAT(proc_args) => nfsproc3_fsstat(xid, proc_args, output, context).await,
        NFSv3_args::FSINFO(proc_args) => nfsproc3_fsinfo(xid, proc_args, output, context).await,
        NFSv3_args::PATHCONF(proc_args) => nfsproc3_pathconf(xid, proc_args, output, context).await,
        NFSv3_args::COMMIT(proc_args) => nfsproc3_commit(xid, proc_args, output, context).await,
        NFSv3_args::INVALID => {
            warn!("Invalid NFS operation");
            xdr::rpc::proc_unavail_reply_message(xid).serialize(output)?;
            Ok(())
        }
    }
}
