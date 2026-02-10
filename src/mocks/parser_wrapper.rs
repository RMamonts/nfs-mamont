use std::io::{Cursor, Write};

use crate::allocator::Allocator;
use crate::client::arguments::access::access_args;
use crate::client::arguments::commit::commit_args;
use crate::client::arguments::create::create_args;
use crate::client::arguments::fs_info::fs_info_args;
use crate::client::arguments::fs_stat::fs_stat_args;
use crate::client::arguments::get_attr::get_attr3_args;
use crate::client::arguments::link::link_args;
use crate::client::arguments::lookup::lookup_args;
use crate::client::arguments::mk_dir::mk_dir_args;
use crate::client::arguments::mk_node::mk_node_args;
use crate::client::arguments::mount::{mount_args, unmount_args};
use crate::client::arguments::path_conf::path_conf_args;
use crate::client::arguments::read::read_args;
use crate::client::arguments::read_dir::read_dir_args;
use crate::client::arguments::read_dir_plus::read_dir_plus_args;
use crate::client::arguments::read_link::read_link_args;
use crate::client::arguments::remove::remove_args;
use crate::client::arguments::rename::rename_args;
use crate::client::arguments::rm_dir::rm_dir_args;
use crate::client::arguments::set_attr::set_attr_args;
use crate::client::arguments::symlink::symlink_args;
use crate::client::arguments::write::write_args;
use crate::client::results::RpcRequest;
use crate::mocks::fuzz_socket::{FuzzMockSocket, FuzzSocketHandler};
use crate::parser;
use crate::parser::parser_struct::{RpcParser, MAX_MESSAGE_LEN};
use crate::parser::Arguments;

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
                get_attr3_args(&mut self.tmp_buffer, get).unwrap();
            }
            Arguments::SetAttr(set) => {
                set_attr_args(&mut self.tmp_buffer, set).unwrap();
            }
            Arguments::LookUp(lookup) => {
                lookup_args(&mut self.tmp_buffer, lookup).unwrap();
            }
            Arguments::Access(access) => {
                access_args(&mut self.tmp_buffer, access).unwrap();
            }
            Arguments::ReadLink(link) => {
                read_link_args(&mut self.tmp_buffer, link).unwrap();
            }
            Arguments::Read(read) => {
                read_args(&mut self.tmp_buffer, read).unwrap();
            }
            Arguments::Write(write) => {
                write_args(&mut self.tmp_buffer, write).unwrap();
            }
            Arguments::Create(create) => {
                create_args(&mut self.tmp_buffer, create).unwrap();
            }
            Arguments::MkDir(mkdir) => {
                mk_dir_args(&mut self.tmp_buffer, mkdir).unwrap();
            }
            Arguments::SymLink(symlink) => {
                symlink_args(&mut self.tmp_buffer, symlink).unwrap();
            }
            Arguments::MkNod(mknod) => {
                mk_node_args(&mut self.tmp_buffer, mknod).unwrap();
            }
            Arguments::Remove(remove) => {
                remove_args(&mut self.tmp_buffer, remove).unwrap();
            }
            Arguments::RmDir(rmdir) => {
                rm_dir_args(&mut self.tmp_buffer, rmdir).unwrap();
            }
            Arguments::Rename(rename) => {
                rename_args(&mut self.tmp_buffer, rename).unwrap();
            }
            Arguments::Link(link) => {
                link_args(&mut self.tmp_buffer, link).unwrap();
            }
            Arguments::ReadDir(read_dir) => {
                read_dir_args(&mut self.tmp_buffer, read_dir).unwrap();
            }
            Arguments::ReadDirPlus(read_dir_plus) => {
                read_dir_plus_args(&mut self.tmp_buffer, read_dir_plus).unwrap();
            }
            Arguments::FsStat(fs_stat) => {
                fs_stat_args(&mut self.tmp_buffer, fs_stat).unwrap();
            }
            Arguments::FsInfo(fs_info) => {
                fs_info_args(&mut self.tmp_buffer, fs_info).unwrap();
            }
            Arguments::PathConf(path) => {
                path_conf_args(&mut self.tmp_buffer, path).unwrap();
            }
            Arguments::Commit(commit) => {
                commit_args(&mut self.tmp_buffer, commit).unwrap();
            }
            Arguments::Mount(mount) => {
                mount_args(&mut self.tmp_buffer, mount).unwrap();
            }
            Arguments::Unmount(unmount) => {
                unmount_args(&mut self.tmp_buffer, unmount).unwrap();
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
    pub async fn parse_message(&mut self) -> parser::Result<Box<Arguments>> {
        self.parser.parse_message().await
    }
}
