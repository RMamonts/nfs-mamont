use std::io::{Cursor, Write};

use arbitrary::{Arbitrary, Unstructured};
use nfs_mamont::allocator::Allocator;
use nfs_mamont::client::arguments::{
    access, commit, create, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node, mount,
    path_conf, read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink,
    write,
};
use nfs_mamont::mocks::fuzz_socket::{FuzzMockSocket, FuzzSocketHandler};
use nfs_mamont::mount::MOUNT_PROGRAM;
use nfs_mamont::nfsv3::NFS_PROGRAM;
use nfs_mamont::parser::parser_struct::RpcParser;
use nfs_mamont::parser::parser_struct::MAX_MESSAGE_LEN;
use nfs_mamont::parser::{Arguments, Result};

#[derive(Clone, Debug)]
pub struct RpcRequest {
    // calculated
    pub size: u32,
    pub xid: u32,
    // always
    pub request: u32,
    // always 2
    pub rpc_version: u32,
    pub prog: u32,
    // both mount and nfs - 3
    pub version: u32,
    pub proc: u32,
    // for now only None (0)
    pub auth: u32,
    // for now only None (0)
    pub auth_verf: u32,
    pub args: Arguments,
}

impl<'a> Arbitrary<'a> for RpcRequest {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let prog = u.int_in_range(NFS_PROGRAM..=MOUNT_PROGRAM)?;
        let proc = u.int_in_range(0..=24)?;
        let args = match (prog, proc) {
            (NFS_PROGRAM, 0) => Arguments::Null,
            (NFS_PROGRAM, 1) => Arguments::GetAttr(u.arbitrary()?),
            (NFS_PROGRAM, 2) => Arguments::SetAttr(u.arbitrary()?),
            (NFS_PROGRAM, 3) => Arguments::LookUp(u.arbitrary()?),
            (NFS_PROGRAM, 4) => Arguments::Access(u.arbitrary()?),
            (NFS_PROGRAM, 5) => Arguments::ReadLink(u.arbitrary()?),
            (NFS_PROGRAM, 6) => Arguments::Read(u.arbitrary()?),
            (NFS_PROGRAM, 7) => Arguments::Write(u.arbitrary()?),
            (NFS_PROGRAM, 8) => Arguments::Create(u.arbitrary()?),
            (NFS_PROGRAM, 9) => Arguments::MkDir(u.arbitrary()?),
            (NFS_PROGRAM, 10) => Arguments::SymLink(u.arbitrary()?),
            (NFS_PROGRAM, 11) => Arguments::MkNod(u.arbitrary()?),
            (NFS_PROGRAM, 12) => Arguments::Remove(u.arbitrary()?),
            (NFS_PROGRAM, 13) => Arguments::RmDir(u.arbitrary()?),
            (NFS_PROGRAM, 14) => Arguments::Rename(u.arbitrary()?),
            (NFS_PROGRAM, 15) => Arguments::Link(u.arbitrary()?),
            (NFS_PROGRAM, 16) => Arguments::ReadDir(u.arbitrary()?),
            (NFS_PROGRAM, 17) => Arguments::ReadDirPlus(u.arbitrary()?),
            (NFS_PROGRAM, 18) => Arguments::FsStat(u.arbitrary()?),
            (NFS_PROGRAM, 19) => Arguments::FsInfo(u.arbitrary()?),
            (NFS_PROGRAM, 20) => Arguments::PathConf(u.arbitrary()?),
            (NFS_PROGRAM, 21) => Arguments::Commit(u.arbitrary()?),
            (MOUNT_PROGRAM, 0) => Arguments::Null,
            (MOUNT_PROGRAM, 1) => Arguments::Mount(u.arbitrary()?),
            (MOUNT_PROGRAM, 2) => Arguments::Dump,
            (MOUNT_PROGRAM, 3) => Arguments::Unmount(u.arbitrary()?),
            (MOUNT_PROGRAM, 4) => Arguments::UnmountAll,
            (MOUNT_PROGRAM, 5) => Arguments::Export,
            _ => u.arbitrary::<Arguments>()?,
        };
        Ok(Self {
            size: 0,
            xid: 0,
            request: 0,
            //so there would be RpcVersionMismatch
            rpc_version: u.int_in_range(1..=2)?,
            //so there would be ProgramMismatch
            prog,
            //so there would be ProgramVersionMismatch
            version: u.int_in_range(2..=3)?,
            //so there would be ProcedureMismatch (nfsv3 has 21 proc)
            proc,
            auth: 0,
            auth_verf: 0,
            args,
        })
    }
}

pub struct ParserWrapper<A: Allocator> {
    parser: RpcParser<A, FuzzMockSocket>,
    sender: FuzzSocketHandler,
    tmp_buffer: Cursor<Vec<u8>>,
}

impl<A: Allocator> ParserWrapper<A> {
    pub fn new(parser: RpcParser<A, FuzzMockSocket>, sender: FuzzSocketHandler) -> Self {
        // inner buffer size + max amount of bytes can be generated for Slice
        Self { parser, sender, tmp_buffer: Cursor::new(vec![0u8; MAX_MESSAGE_LEN + 1000]) }
    }
    // forms completely new message
    pub fn write_new_message(&mut self, arg: RpcRequest) {
        self.tmp_buffer.set_position(36);
        match arg.args {
            Arguments::GetAttr(get) => {
                get_attr::get_attr_args(&mut self.tmp_buffer, get).unwrap();
            }
            Arguments::SetAttr(set) => {
                set_attr::set_attr_args(&mut self.tmp_buffer, set).unwrap();
            }
            Arguments::LookUp(lookup) => {
                lookup::lookup_args(&mut self.tmp_buffer, lookup).unwrap();
            }
            Arguments::Access(access) => {
                access::access_args(&mut self.tmp_buffer, access).unwrap();
            }
            Arguments::ReadLink(link) => {
                read_link::read_link_args(&mut self.tmp_buffer, link).unwrap();
            }
            Arguments::Read(read) => {
                read::read_args(&mut self.tmp_buffer, read).unwrap();
            }
            Arguments::Write(write) => {
                write::write_args(&mut self.tmp_buffer, write).unwrap();
            }
            Arguments::Create(create) => {
                create::create_args(&mut self.tmp_buffer, create).unwrap();
            }
            Arguments::MkDir(mkdir) => {
                mk_dir::mk_dir_args(&mut self.tmp_buffer, mkdir).unwrap();
            }
            Arguments::SymLink(symlink) => {
                symlink::symlink_args(&mut self.tmp_buffer, symlink).unwrap();
            }
            Arguments::MkNod(mknod) => {
                mk_node::mk_node_args(&mut self.tmp_buffer, mknod).unwrap();
            }
            Arguments::Remove(remove) => {
                remove::remove_args(&mut self.tmp_buffer, remove).unwrap();
            }
            Arguments::RmDir(rmdir) => {
                rm_dir::rm_dir_args(&mut self.tmp_buffer, rmdir).unwrap();
            }
            Arguments::Rename(rename) => {
                rename::rename_args(&mut self.tmp_buffer, rename).unwrap();
            }
            Arguments::Link(link) => {
                link::link_args(&mut self.tmp_buffer, link).unwrap();
            }
            Arguments::ReadDir(read_dir) => {
                read_dir::read_dir_args(&mut self.tmp_buffer, read_dir).unwrap();
            }
            Arguments::ReadDirPlus(read_dir_plus) => {
                read_dir_plus::read_dir_plus_args(&mut self.tmp_buffer, read_dir_plus).unwrap();
            }
            Arguments::FsStat(fs_stat) => {
                fs_stat::fs_stat_args(&mut self.tmp_buffer, fs_stat).unwrap();
            }
            Arguments::FsInfo(fs_info) => {
                fs_info::fs_info_args(&mut self.tmp_buffer, fs_info).unwrap();
            }
            Arguments::PathConf(path) => {
                path_conf::path_conf_args(&mut self.tmp_buffer, path).unwrap();
            }
            Arguments::Commit(commit) => {
                commit::commit_args(&mut self.tmp_buffer, commit).unwrap();
            }
            Arguments::Mount(mount) => {
                mount::mount_args(&mut self.tmp_buffer, mount).unwrap();
            }
            Arguments::Unmount(unmount) => {
                mount::unmount_args(&mut self.tmp_buffer, unmount).unwrap();
            }
            // though, apparently there is no difference between nfsv3 null and mount null
            Arguments::Null | Arguments::Export | Arguments::Dump | Arguments::UnmountAll => {}
        };
        let pos = self.tmp_buffer.position() as usize;
        let size = ((self.tmp_buffer.position() as u32) | 0x8000_0000).to_be_bytes();
        self.tmp_buffer.set_position(0);
        // size of Rpc message
        self.tmp_buffer.write_all(&size).unwrap();
        // xid
        self.tmp_buffer.write_all(&arg.xid.to_be_bytes()).unwrap();
        // call/reply
        self.tmp_buffer.write_all(&arg.request.to_be_bytes()).unwrap();
        // rpc_version
        self.tmp_buffer.write_all(&arg.rpc_version.to_be_bytes()).unwrap();
        // program
        self.tmp_buffer.write_all(&arg.prog.to_be_bytes()).unwrap();
        // program version
        self.tmp_buffer.write_all(&arg.version.to_be_bytes()).unwrap();
        // procedure
        self.tmp_buffer.write_all(&arg.proc.to_be_bytes()).unwrap();
        // now we can do only Auth::None
        self.tmp_buffer.write_all(&arg.auth.to_be_bytes()).unwrap();
        // now we can do only Auth::None
        self.tmp_buffer.write_all(&arg.auth_verf.to_be_bytes()).unwrap();
        assert_eq!(self.tmp_buffer.position(), 36);
        self.tmp_buffer.set_position(0);
        // there should be sending to mpsc
        self.sender.send_data(self.tmp_buffer.get_ref()[..pos].to_vec());
    }
    pub async fn parse_message(&mut self) -> Result<Box<Arguments>> {
        self.parser.parse_message().await
    }
}
