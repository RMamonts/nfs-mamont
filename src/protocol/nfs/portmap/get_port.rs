//! Implementation of the GETPORT procedure (procedure 3) for PORTMAP protocol
//! as defined in RFC 5531 (previously RFC 1057 Appendix A).
//!
//! The GETPORT procedure maps an RPC program number, version number, and transport protocol
//! to the port number on which the program is awaiting call requests. It takes a map structure
//! containing the program number, version number, protocol number, and dummy port and returns
//! the port number where that combination can be found.
//!
//! This procedure is essential for clients to discover the dynamic port numbers assigned to
//! various RPC services such as NFS, MOUNT, and others.

use std::io::{Read, Write};

use tracing::debug;

use crate::protocol::rpc;
use crate::protocol::xdr::{self, XDR};

/// Handles PORTMAP protocol GETPORT procedure (procedure 3)
///
/// GETPORT maps an RPC program and version to a TCP/UDP port.
/// Takes a mapping request with program number, version, protocol and port.
/// Returns the port number where the requested service can be reached.
///
/// NOTE: Fake function. Always direct back to the same host port
///
/// # Arguments
///
/// * `xid` - RPC transaction ID
/// * `read` - Input stream containing the port mapping request
/// * `output` - Output stream for writing the response
/// * `context` - Server context containing port information
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok(()) on success or an error
pub fn pmapproc_getport(
    xid: u32,
    read: &mut impl Read,
    output: &mut impl Write,
    context: &rpc::Context,
) -> Result<(), anyhow::Error> {
    let mut mapping = xdr::portmap::mapping::default();
    mapping.deserialize(read)?;
    debug!("pmapproc_getport({:?}, {:?}) ", xid, mapping);
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    let port = context.local_port as u32;
    debug!("\t{:?} --> {:?}", xid, port);
    port.serialize(output)?;
    Ok(())
}
