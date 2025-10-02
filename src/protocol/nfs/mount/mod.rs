//! `MOUNT` protocol implementation for NFS version 3 as specified in RFC 1813 section 5.0.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.0>.

use std::io;
use std::io::Write;

use crate::protocol::rpc;
use tracing::warn;

mod dump;
mod export;
mod mnt;
mod null;
mod umnt;
mod umnt_all;

use crate::xdr::mount::Args;

use dump::mountproc3_dump;
use export::mountproc3_export;
use mnt::mountproc3_mnt;
use null::mountproc3_null;
use umnt::mountproc3_umnt;
use umnt_all::mountproc3_umnt_all;

/// Checks if a requested path matches an export path with proper path separator handling.
///
/// This function prevents incorrect matches like `/data2/file` matching `/data` export
/// by ensuring that after the export name prefix, we either have the end of the string
/// or a path separator (`/`).
///
/// # Arguments
///
/// * `requested_path` - The path being requested by the client
/// * `export_name` - The export path to match against
///
/// # Returns
///
/// * `bool` - true if the requested path properly matches the export
fn matches_export_path(requested_path: &str, export_name: &str) -> bool {
    if requested_path == export_name {
        // Exact match
        true
    } else if export_name == "/" && requested_path.starts_with("/") {
        // Special case: root export matches any absolute path
        true
    } else if requested_path.starts_with(export_name)
        && requested_path.chars().nth(export_name.len()) == Some('/')
    {
        // Export name is a prefix and is followed by a path separator
        true
    } else {
        false
    }
}

/// Extracts the machine name from the RPC context's authentication information.
fn machine_name_from_context(context: &rpc::Context) -> Option<String> {
    if let Some(auth) = &context.auth {
        if let Ok(machine_name) = String::from_utf8(auth.machinename.clone()) {
            return Some(machine_name);
        } else {
            warn!("Failed to convert machine name to UTF-8");
        }
    } else {
        warn!("No auth information in context to extract machine name");
    }
    None
}

/// Main handler for `MOUNT` procedures of version 3 protocol.
///
/// # Arguments
///
/// * `xid` - RPC transaction ID from the client
/// * `call` - The RPC call body containing program, version, and procedure numbers
/// * `input` - Input stream for reading procedure arguments
/// * `output` - Output stream for writing procedure results
/// * `context` - Server context containing exports and VFS information
///
/// # Returns
///
/// * `io::Result<()>` - Ok(()) on success or an error
pub async fn handle_mount(
    xid: u32,
    arg: Box<Args>,
    output: &mut impl Write,
    context: &rpc::Context,
) -> io::Result<()> {
    match *arg {
        Args::Null => mountproc3_null(xid, output),
        Args::Mnt(proc_args) => mountproc3_mnt(xid, proc_args, output, context).await,
        Args::Dump => mountproc3_dump(xid, output, context).await,
        Args::Umnt(proc_args) => mountproc3_umnt(xid, proc_args, output, context).await,
        Args::Umntall => mountproc3_umnt_all(xid, output, context).await,
        Args::Export => mountproc3_export(xid, output, context).await,
    }
}
