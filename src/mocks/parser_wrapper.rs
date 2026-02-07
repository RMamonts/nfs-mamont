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
use crate::mocks::fuzz_socket::{FuzzMockSocket, FuzzSocketHandler};
use crate::mount::MOUNT_PROGRAM;
use crate::nfsv3::NFS_PROGRAM;
use crate::parser::parser_struct::{RpcParser, MAX_MESSAGE_LEN};
use crate::parser::Arguments;

pub struct ParserWrapper<A: Allocator> {
    parser: RpcParser<A, FuzzMockSocket>,
    sender: FuzzSocketHandler,
    tmp_buffer: Cursor<Vec<u8>>,
}

impl<A: Allocator> ParserWrapper<A> {
    pub fn new(parser: RpcParser<A, FuzzMockSocket>, sender: FuzzSocketHandler) -> Self {
        Self { parser, sender, tmp_buffer: Cursor::new(vec![0u8; MAX_MESSAGE_LEN + 10000]) }
    }
    // forms completely new message
    pub fn write_new_message(&mut self, arg: Arguments) {
        self.tmp_buffer.set_position(36);
        let (prog, proc) = match arg {
            Arguments::GetAttr(get) => {
                get_attr3_args(&mut self.tmp_buffer, get).unwrap();
                (NFS_PROGRAM, 1_u32)
            }
            Arguments::SetAttr(set) => {
                set_attr_args(&mut self.tmp_buffer, set).unwrap();
                (NFS_PROGRAM, 2)
            }
            Arguments::LookUp(lookup) => {
                lookup_args(&mut self.tmp_buffer, lookup).unwrap();
                (NFS_PROGRAM, 3)
            }
            Arguments::Access(access) => {
                access_args(&mut self.tmp_buffer, access).unwrap();
                (NFS_PROGRAM, 4)
            }
            Arguments::ReadLink(link) => {
                read_link_args(&mut self.tmp_buffer, link).unwrap();
                (NFS_PROGRAM, 5)
            }
            Arguments::Read(read) => {
                read_args(&mut self.tmp_buffer, read).unwrap();
                (NFS_PROGRAM, 6)
            }
            Arguments::Write(write) => {
                write_args(&mut self.tmp_buffer, write).unwrap();
                (NFS_PROGRAM, 7)
            }
            Arguments::Create(create) => {
                create_args(&mut self.tmp_buffer, create).unwrap();
                (NFS_PROGRAM, 8)
            }
            Arguments::MkDir(mkdir) => {
                mk_dir_args(&mut self.tmp_buffer, mkdir).unwrap();
                (NFS_PROGRAM, 9)
            }
            Arguments::SymLink(symlink) => {
                symlink_args(&mut self.tmp_buffer, symlink).unwrap();
                (NFS_PROGRAM, 10)
            }
            Arguments::MkNod(mknod) => {
                mk_node_args(&mut self.tmp_buffer, mknod).unwrap();
                (NFS_PROGRAM, 11)
            }
            Arguments::Remove(remove) => {
                remove_args(&mut self.tmp_buffer, remove).unwrap();
                (NFS_PROGRAM, 12)
            }
            Arguments::RmDir(rmdir) => {
                rm_dir_args(&mut self.tmp_buffer, rmdir).unwrap();
                (NFS_PROGRAM, 13)
            }
            Arguments::Rename(rename) => {
                rename_args(&mut self.tmp_buffer, rename).unwrap();
                (NFS_PROGRAM, 14)
            }
            Arguments::Link(link) => {
                link_args(&mut self.tmp_buffer, link).unwrap();
                (NFS_PROGRAM, 15)
            }
            Arguments::ReadDir(read_dir) => {
                read_dir_args(&mut self.tmp_buffer, read_dir).unwrap();
                (NFS_PROGRAM, 16)
            }
            Arguments::ReadDirPlus(read_dir_plus) => {
                read_dir_plus_args(&mut self.tmp_buffer, read_dir_plus).unwrap();
                (NFS_PROGRAM, 17)
            }
            Arguments::FsStat(fs_stat) => {
                fs_stat_args(&mut self.tmp_buffer, fs_stat).unwrap();
                (NFS_PROGRAM, 18)
            }
            Arguments::FsInfo(fs_info) => {
                fs_info_args(&mut self.tmp_buffer, fs_info).unwrap();
                (NFS_PROGRAM, 19)
            }
            Arguments::PathConf(path) => {
                path_conf_args(&mut self.tmp_buffer, path).unwrap();
                (NFS_PROGRAM, 20)
            }
            Arguments::Commit(commit) => {
                commit_args(&mut self.tmp_buffer, commit).unwrap();
                (NFS_PROGRAM, 21)
            }
            Arguments::Mount(mount) => {
                mount_args(&mut self.tmp_buffer, mount).unwrap();
                (MOUNT_PROGRAM, 1)
            }
            Arguments::Unmount(unmount) => {
                unmount_args(&mut self.tmp_buffer, unmount).unwrap();
                (MOUNT_PROGRAM, 3)
            }
            // though, apparently there is no difference between nfsv3 null and mount null
            Arguments::Null => (NFS_PROGRAM, 0),
            Arguments::Export => (MOUNT_PROGRAM, 5),
            Arguments::Dump => (MOUNT_PROGRAM, 2),
            Arguments::UnmountAll => (MOUNT_PROGRAM, 4),
        };
        let pos = self.tmp_buffer.position() as usize;
        let size = ((self.tmp_buffer.position() as u32) | 0x8000_0000).to_be_bytes();
        self.tmp_buffer.set_position(0);
        self.tmp_buffer.write(&size).unwrap();
        self.tmp_buffer
            .write(&[0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02])
            .unwrap();
        self.tmp_buffer.write(&prog.to_be_bytes()).unwrap();
        self.tmp_buffer.write(&[0, 0, 0, 3]).unwrap();
        self.tmp_buffer.write(&proc.to_be_bytes()).unwrap();
        // now we can do only Auth::None
        self.tmp_buffer.write(&[0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(self.tmp_buffer.position(), 36);
        self.tmp_buffer.set_position(0);
        // there should be sending to mpsc
        self.sender.send_data(self.tmp_buffer.get_ref()[..pos].to_vec());
    }
    pub async fn parse_message(&mut self) -> Box<Arguments> {
        self.parser.parse_message().await.unwrap()
    }
}
