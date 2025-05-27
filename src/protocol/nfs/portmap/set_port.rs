use std::io::{Read, Write};

use crate::protocol::nfs::portmap::PortmapKey;
use crate::protocol::rpc::Context;
use crate::xdr;
use crate::xdr::portmap::mapping;
use crate::xdr::{deserialize, Serialize};

/// Handles the Portmap SETPORT procedure to register a new port mapping
///
/// # Arguments
/// * `xid` - XID (Transaction ID) for RPC message
/// * `read` - Input stream to read the mapping request from
/// * `output` - Output stream to write the response to
/// * `context` - Shared RPC context containing the portmap table
///
/// # Returns
/// `Result<(), anyhow::Error>` indicating success or failure
///
/// # Behavior
/// 1. Deserializes the mapping request
/// 2. Checks if the mapping already exists
/// 3. If not exists, adds the new mapping
/// 4. Sends success response with boolean result (true = added, false = existed)
pub fn pmapproc_setport(
    xid: u32,
    read: &mut impl Read,
    output: &mut impl Write,
    context: &mut Context,
) -> Result<(), anyhow::Error> {
    let mapping = deserialize::<mapping>(read)?;
    let entry = PortmapKey { prog: mapping.prog, vers: mapping.vers, prot: mapping.prot };
    let mut binding = context.portmap_table.write().unwrap();
    let port = binding.table.get(&entry).copied();
    let result = match port {
        None => {
            binding.table.insert(entry, mapping.port as u16);
            true
        }
        Some(_) => false,
    };
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    result.serialize(output)?;
    Ok(())
}
