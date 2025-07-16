use std::io::{Read, Write};
use crate::protocol::nfs::portmap::{get_port, PortmapKey};
use crate::protocol::rpc::Context;
use crate::xdr;
use crate::xdr::{deserialize, Serialize};
use crate::xdr::portmap::{mapping, IPPROTO_TCP, IPPROTO_UDP};

pub fn pmapproc_unsetport(
    xid: u32,
    read: &mut impl Read,
    output: &mut impl Write,
    context: &Context,
) -> Result<(), anyhow::Error> {
    let mapping = deserialize::<mapping>(read)?;
    let entries = [PortmapKey { prog: mapping.prog, vers: mapping.vers, prot: IPPROTO_TCP },
                                    PortmapKey { prog: mapping.prog, vers: mapping.vers, prot: IPPROTO_UDP }];
    let mut binding = context
        .portmap_table
        .write()
        .unwrap();
    let mut result = false;
    for entry in &entries {
        let deletion = match binding.table.remove(entry) {
            Some(_) => true,
            None => false
        };
        result = result || deletion;
    }
    assert!(binding.table.get(&entries[0]).is_none());
    assert!(binding.table.get(&entries[1]).is_none());
    xdr::rpc::make_success_reply(xid).serialize(output)?;
    result.serialize(output)?;
    Ok(())
}
