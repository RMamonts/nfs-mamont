use std::io::Write;

use crate::protocol::rpc::Context;
use crate::xdr;
use crate::xdr::portmap::{mapping, pmaplist};
use crate::xdr::Serialize;

/// Implements PMAPPROC_DUMP operation from RFC 1057 (Port Mapper Protocol)
/// Returns all entries from the port mapper's registration table
///
/// # Arguments
/// * `xid` - XID (Transaction ID) for RPC message
/// * `output` - Writer for serialized XDR response
/// * `context` - Shared server context containing portmap table
///
/// # Returns
/// Result indicating success or failure of the operation
///
/// # XDR Protocol Notes (RFC 4506)
/// 1. Response format is:
///    - RPC reply header (success/failure)
///    - pmaplist (linked list of mappings)
/// 2. Empty list is represented by zero-length array
pub fn pmapproc_dump(
    xid: u32,
    output: &mut impl Write,
    context: &Context,
) -> Result<(), anyhow::Error> {
    let binding = context.portmap_table.read().unwrap();
    let entries: Vec<mapping> = binding
        .table
        .iter()
        .map(|(entry, port)| mapping {
            prog: entry.prog,
            vers: entry.vers,
            prot: entry.prot,
            port: *port as u32,
        })
        .collect();
    drop(binding);
    let result = {
        let mut list_head = None;
        for map in entries.iter().rev() {
            list_head = Some(pmaplist { map: *map, next: Box::from(list_head) });
        }
        list_head
    };

    xdr::rpc::make_success_reply(xid).serialize(output)?;

    if let Some(list) = result {
        let sent = Some(list);
        sent.serialize(output)?;
    } else {
        0_u32.serialize(output)?;
    }
    Ok(())
}
