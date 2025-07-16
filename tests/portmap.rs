use std::io::Cursor;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use num_traits::ToPrimitive;

use nfs_mamont::protocol::nfs::portmap::PortmapTable;
use nfs_mamont::protocol::rpc;
use nfs_mamont::protocol::rpc::Context;
use nfs_mamont::vfs::{Capabilities, ReadDirResult};
use nfs_mamont::xdr::nfs3::{
    fattr3, fileid3, filename3, ftype3, nfspath3, nfsstat3, sattr3, specdata3,
};
use nfs_mamont::xdr::portmap::mapping;
use nfs_mamont::xdr::rpc::call_body;
use nfs_mamont::xdr::{deserialize, nfs3, Serialize};
use nfs_mamont::{vfs, xdr};

pub struct DemoFS {
    pub root: String,
}

const RPC_MSG_SIZE: u64 = 24;
const OUTPUT_SIZE: usize = 28;
const INPUT_SIZE: usize = 16;

#[async_trait]
impl vfs::NFSFileSystem for DemoFS {
    fn generation(&self) -> u64 {
        unimplemented!()
    }

    fn capabilities(&self) -> Capabilities {
        unimplemented!()
    }

    fn root_dir(&self) -> fileid3 {
        unimplemented!()
    }

    async fn lookup(&self, _dirid: fileid3, _filename: &filename3) -> Result<fileid3, nfsstat3> {
        unimplemented!()
    }

    async fn getattr(&self, _id: fileid3) -> Result<fattr3, nfsstat3> {
        unimplemented!()
    }

    async fn setattr(&self, _id: fileid3, _setattr: sattr3) -> Result<fattr3, nfsstat3> {
        unimplemented!()
    }

    async fn read(
        &self,
        _id: fileid3,
        _offset: u64,
        _count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        unimplemented!()
    }

    async fn write(&self, _id: fileid3, _offset: u64, _data: &[u8]) -> Result<fattr3, nfsstat3> {
        unimplemented!()
    }

    async fn create(
        &self,
        _dirid: fileid3,
        _filename: &filename3,
        _attr: sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        unimplemented!()
    }

    async fn create_exclusive(
        &self,
        _dirid: fileid3,
        _filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
        unimplemented!()
    }

    async fn mkdir(
        &self,
        _dirid: fileid3,
        _dirname: &filename3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        unimplemented!()
    }

    async fn remove(&self, _dirid: fileid3, _filename: &filename3) -> Result<(), nfsstat3> {
        unimplemented!()
    }

    async fn rename(
        &self,
        _from_dirid: fileid3,
        _from_filename: &filename3,
        _to_dirid: fileid3,
        _to_filename: &filename3,
    ) -> Result<(), nfsstat3> {
        unimplemented!()
    }

    async fn readdir(
        &self,
        _dirid: fileid3,
        _start_after: fileid3,
        _max_entries: usize,
    ) -> Result<ReadDirResult, nfsstat3> {
        unimplemented!()
    }

    async fn symlink(
        &self,
        _dirid: fileid3,
        _linkname: &filename3,
        _symlink: &nfspath3,
        _attr: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        unimplemented!()
    }

    async fn readlink(&self, _id: fileid3) -> Result<nfspath3, nfsstat3> {
        unimplemented!()
    }

    async fn link(
        &self,
        _file_id: fileid3,
        _link_dir_id: fileid3,
        _link_name: &filename3,
    ) -> Result<fattr3, nfsstat3> {
        unimplemented!()
    }

    async fn mknod(
        &self,
        _dir_id: fileid3,
        _name: &filename3,
        _ftype: ftype3,
        _specdata: specdata3,
        _attrs: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        unimplemented!()
    }

    async fn commit(
        &self,
        _file_id: fileid3,
        _offset: u64,
        _count: u32,
    ) -> Result<fattr3, nfsstat3> {
        unimplemented!()
    }
}

fn multiple_mappings(amount: u32) -> Vec<mapping> {
    let mut result = Vec::<mapping>::with_capacity(amount as usize);
    for i in 1..=amount / 2 {
        result.push(mapping { prog: i, vers: 1, prot: 0, port: i + 1000 })
    }
    for i in 1..=amount / 2 {
        result.push(mapping { prog: i, vers: 2, prot: 0, port: i + 2000 })
    }
    result
}

fn multiple_contexts(amount: u32) -> Vec<Context> {
    let mut result = Vec::<Context>::with_capacity(amount as usize);
    let table = Arc::from(RwLock::from(PortmapTable::default()));
    for i in 1..=amount {
        result.push(Context {
            local_port: 0,
            client_addr: i.to_string(),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { root: "root".to_string() }),
            mount_signal: None,
            export_name: Arc::from("/".to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: table.clone(),
        });
    }
    result
}

fn send_get_port(
    mut context: &mut Context,
    input: &mut Cursor<Vec<u8>>,
    output: &mut Cursor<Vec<u8>>,
    mapping_args: mapping,
) -> Result<(), anyhow::Error> {
    let body = call_body {
        rpcvers: 2,
        prog: xdr::portmap::PROGRAM,
        vers: xdr::portmap::VERSION,
        proc: xdr::portmap::PortmapProgram::PMAPPROC_GETPORT.to_u32().unwrap(),
        cred: Default::default(),
        verf: Default::default(),
    };

    mapping_args.serialize(input)?;
    input.set_position(0);
    nfs_mamont::protocol::nfs::portmap::handle_portmap(
        u32::default(),
        &body,
        input,
        output,
        &mut context,
    )
    .expect("can't proceed get_port");
    Ok(())
}

fn send_set_port(
    mut context: &mut Context,
    input: &mut Cursor<Vec<u8>>,
    output: &mut Cursor<Vec<u8>>,
    mapping_args: mapping,
) -> Result<(), anyhow::Error> {
    let body = call_body {
        rpcvers: 2,
        prog: xdr::portmap::PROGRAM,
        vers: xdr::portmap::VERSION,
        proc: xdr::portmap::PortmapProgram::PMAPPROC_SET.to_u32().unwrap(),
        cred: Default::default(),
        verf: Default::default(),
    };
    mapping_args.serialize(input)?;

    input.set_position(0);
    nfs_mamont::protocol::nfs::portmap::handle_portmap(
        u32::default(),
        &body,
        input,
        output,
        &mut context,
    )
    .expect("can't proceed get_port");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// simple test to assure, that result of GET_PORT operation is zero,
    /// when there is no attached port to corresponding program
    fn get_port_zero_reply(port: u16) {
        let mut context = Context {
            local_port: 0,
            client_addr: "1".to_string(),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { root: "root".to_string() }),
            mount_signal: None,
            export_name: Arc::from("/".to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
        };
        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));
        let mapping_args = mapping {
            prog: nfs3::PROGRAM,
            vers: nfs3::VERSION,
            prot: xdr::portmap::IPPROTO_TCP,
            port: port as u32,
        };
        send_get_port(&mut context, &mut input, &mut output, mapping_args)
            .expect("failed to proceed request");
        output.set_position(RPC_MSG_SIZE);
        let result = deserialize::<u32>(&mut output).expect("can't proceed resulted port");
        assert_eq!(result, 0);
    }

    ///simple test to assure, that after SET_PORT operation for program without
    /// associated port, entry creates and result of operation is TRUE
    fn set_port_ok_reply(port: u16) {
        let mut context = Context {
            local_port: 0,
            client_addr: "1".to_string(),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { root: "root".to_string() }),
            mount_signal: None,
            export_name: Arc::from("/".to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
        };
        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));
        let mapping_args = mapping {
            prog: nfs3::PROGRAM,
            vers: nfs3::VERSION,
            prot: xdr::portmap::IPPROTO_TCP,
            port: port as u32,
        };
        send_set_port(&mut context, &mut input, &mut output, mapping_args)
            .expect("failed to proceed request");
        input.set_position(0);
        output.set_position(0);
        send_get_port(&mut context, &mut input, &mut output, mapping_args)
            .expect("failed to proceed request");
        output.set_position(RPC_MSG_SIZE);
        let result = deserialize::<u32>(&mut output).expect("can't proceed resulted port");
        assert_eq!(result, port as u32);
    }

    ///simple test of GET_PORT after SET_PORT
    fn get_port_ok_reply(port: u16) {
        let mapping_args = mapping {
            prog: nfs3::PROGRAM,
            vers: nfs3::VERSION,
            prot: xdr::portmap::IPPROTO_TCP,
            port: port as u32,
        };
        let mut context = Context {
            local_port: 0,
            client_addr: "1".to_string(),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { root: "root".to_string() }),
            mount_signal: None,
            export_name: Arc::from("/".to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
        };
        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));
        send_set_port(&mut context, &mut input, &mut output, mapping_args)
            .expect("failed to proceed request");
        output.set_position(0);
        input.set_position(0);
        send_get_port(&mut context, &mut input, &mut output, mapping_args)
            .expect("failed to proceed request");
        output.set_position(RPC_MSG_SIZE);
        let result = deserialize::<u32>(&mut output).expect("can't proceed resulted port");
        assert_eq!(result, port as u32);
    }

    ///test of multiple GET_PORT after SET_PORT
    fn set_and_get_multiple(amount: u32) {
        let maps = multiple_mappings(amount);
        let mut context = Context {
            local_port: 0,
            client_addr: "1".to_string(),
            auth: xdr::rpc::auth_unix::default(),
            vfs: Arc::new(DemoFS { root: "root".to_string() }),
            mount_signal: None,
            export_name: Arc::from("/".to_string()),
            transaction_tracker: Arc::new(rpc::TransactionTracker::new(Duration::from_secs(60))),
            portmap_table: Arc::from(RwLock::from(PortmapTable::default())),
        };
        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));

        for mapping_arg in maps.clone() {
            input.set_position(0);
            output.set_position(0);
            send_set_port(&mut context, &mut input, &mut output, mapping_arg)
                .expect("failed to proceed request");
            output.set_position(RPC_MSG_SIZE);
            let result = deserialize::<bool>(&mut output).expect("can't proceed resulted port");
            assert_eq!(result, true);
        }

        for mapping_arg in maps {
            input.set_position(0);
            output.set_position(0);
            send_get_port(&mut context, &mut input, &mut output, mapping_arg)
                .expect("failed to proceed request");
            output.set_position(RPC_MSG_SIZE);
            let result = deserialize::<u32>(&mut output).expect("can't proceed resulted port");
            assert_eq!(mapping_arg.prog + mapping_arg.vers * 1000, result);
        }
    }

    ///test of multiple operations asynchronously
    fn multi_thread_sequence(amount: usize) {
        let mut contexts = multiple_contexts((amount / 2) as u32);
        let mappings = multiple_mappings(amount as u32);

        let mut data = Vec::new();
        for i in 0..amount / 2 {
            data.push((contexts[i].clone(), (mappings[i], mappings[amount / 2 + i])));
        }

        std::thread::scope(|scope| {
            for mut d in data {
                scope.spawn(move || {
                    let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
                    let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));
                    send_get_port(&mut d.0, &mut input, &mut output, d.1 .0)
                        .expect("failed to proceed request");
                    input.set_position(0);
                    output.set_position(0);
                    send_set_port(&mut d.0, &mut input, &mut output, d.1 .0)
                        .expect("failed to proceed request");
                    input.set_position(0);
                    output.set_position(0);
                    send_set_port(&mut d.0, &mut input, &mut output, d.1 .1)
                        .expect("failed to proceed request");
                    input.set_position(0);
                    output.set_position(0);
                    send_get_port(&mut d.0, &mut input, &mut output, d.1 .0)
                        .expect("failed to proceed request");
                    input.set_position(0);
                    output.set_position(0);
                    send_get_port(&mut d.0, &mut input, &mut output, d.1 .1)
                        .expect("failed to proceed request");
                });
            }
        });

        let mut input = Cursor::new(Vec::with_capacity(INPUT_SIZE));
        let mut output = Cursor::new(Vec::with_capacity(OUTPUT_SIZE));

        for mapping_arg in mappings {
            input.set_position(0);
            output.set_position(0);
            send_get_port(&mut contexts[0], &mut input, &mut output, mapping_arg)
                .expect("failed to proceed request");
            output.set_position(RPC_MSG_SIZE);
            let result = deserialize::<u32>(&mut output).expect("can't proceed resulted port");
            assert_eq!(mapping_arg.prog + mapping_arg.vers * 1000, result);
        }
    }

    #[test]
    fn get_port_zero_reply_multiple() {
        get_port_zero_reply(0);
        get_port_zero_reply(u16::MAX);
    }

    #[test]
    fn set_port_ok_reply_multiple() {
        set_port_ok_reply(0);
        set_port_ok_reply(u16::MAX);
    }
    #[test]

    fn get_port_ok_reply_multiple() {
        get_port_ok_reply(0);
        get_port_ok_reply(u16::MAX);
    }
    #[test]
    fn multiple_gets_after_sets() {
        set_and_get_multiple(0);
        set_and_get_multiple(789);
    }
    #[test]
    fn multi_threads() {
        multi_thread_sequence(0);
        multi_thread_sequence(100);
    }
}
