//! Implementation of the GETPORT procedure (procedure 3) for port mapper protocol
//! as defined in RFC 1057 A.2 section.
//! https://datatracker.ietf.org/doc/rfc1057/

use std::io::{Read, Write};

use crate::protocol::nfs::portmap::{get_port, PortmapKey};
use crate::protocol::rpc::Context;
use crate::protocol::xdr::{self, deserialize, portmap::mapping, Serialize};

/// Handles the Portmap GETPORT procedure to lookup a registered port mapping
///
/// # Arguments
/// * `xid` - Transaction ID for RPC message correlation
/// * `read` - Input stream to read the lookup request from
/// * `output` - Output stream to write the response to
/// * `context` - Shared RPC context containing the portmap table (read-only)
///
/// # Returns
/// `Result<(), anyhow::Error>` indicating:
/// - `Ok(())` on successful operation
/// - `Err` if deserialization or serialization fails
///
/// # Behavior
/// 1. Deserializes the mapping request from input stream
/// 2. Creates a PortmapKey from the request parameters
/// 3. Looks up the port in the portmap table:
///    - Returns 0 if no mapping exists
///    - Returns the port number if mapping exists
/// 4. Sends RPC success reply with the result
pub fn pmapproc_getport(
    xid: u32,
    read: &mut impl Read,
    output: &mut impl Write,
    context: &Context,
) -> Result<(), anyhow::Error> {
    let mapping = deserialize::<mapping>(read)?;
    let entry = PortmapKey { prog: mapping.prog, vers: mapping.vers, prot: mapping.prot };
    let port = get_port(context, &entry);
    let result = match port {
        None => 0_u32,
        Some(port) => port as u32,
    };
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    result.serialize(output)?;
    Ok(())
}
