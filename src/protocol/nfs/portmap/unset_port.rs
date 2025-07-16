use crate::protocol::nfs::portmap::PortmapKey;
use crate::protocol::rpc::Context;
use crate::xdr;
use crate::xdr::portmap::{mapping, IPPROTO_TCP, IPPROTO_UDP};
use crate::xdr::{deserialize, Serialize};
use std::io::{Read, Write};

pub fn pmapproc_unsetport(
    xid: u32,
    read: &mut impl Read,
    output: &mut impl Write,
    context: &Context,
) -> Result<(), anyhow::Error> {
    let mapping = deserialize::<mapping>(read)?;
    let entries = [
        PortmapKey { prog: mapping.prog, vers: mapping.vers, prot: IPPROTO_TCP },
        PortmapKey { prog: mapping.prog, vers: mapping.vers, prot: IPPROTO_UDP },
    ];
    let mut binding = context.portmap_table.write().unwrap();
    let mut result = false;
    for entry in &entries {
        let deletion = binding.table.remove(entry).is_some();
        result = result || deletion;
    }
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    result.serialize(output)?;
    Ok(())
}
