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
use crate::mount::MOUNT_PROGRAM;
use crate::nfsv3::NFS_PROGRAM;
use crate::parser::parser_struct::{RpcParser, MAX_MESSAGE_LEN};
use crate::parser::Arguments;
use std::io::{Cursor, Write};
use tokio::io::AsyncRead;

pub struct ParserWrapper<A: Allocator, S: AsyncRead + Write + Unpin> {
    parser: RpcParser<A, S>,
}

impl<A: Allocator, S: AsyncRead + Write + Unpin> ParserWrapper<A, S> {
    pub fn new(parser: RpcParser<A, S>) -> Self {
        Self { parser }
    }
    // forms completely new message
    pub fn write_new_message(&mut self, arg: Arguments) {
        let mut tmp_buf = Cursor::new(vec![0u8; MAX_MESSAGE_LEN + 1000]);
        tmp_buf.set_position(36);
        let (prog, proc) = match arg {
            Arguments::GetAttr(get) => {
                get_attr3_args(&mut tmp_buf, get).unwrap();
                (NFS_PROGRAM, 1_u32)
            }
            Arguments::SetAttr(set) => {
                set_attr_args(&mut tmp_buf, set).unwrap();
                (NFS_PROGRAM, 2)
            }
            Arguments::LookUp(lookup) => {
                lookup_args(&mut tmp_buf, lookup).unwrap();
                (NFS_PROGRAM, 3)
            }
            Arguments::Access(access) => {
                access_args(&mut tmp_buf, access).unwrap();
                (NFS_PROGRAM, 4)
            }
            Arguments::ReadLink(link) => {
                read_link_args(&mut tmp_buf, link).unwrap();
                (NFS_PROGRAM, 5)
            }
            Arguments::Read(read) => {
                read_args(&mut tmp_buf, read).unwrap();
                (NFS_PROGRAM, 6)
            }
            Arguments::Write(write) => {
                write_args(&mut tmp_buf, write).unwrap();
                (NFS_PROGRAM, 7)
            }
            Arguments::Create(create) => {
                create_args(&mut tmp_buf, create).unwrap();
                (NFS_PROGRAM, 8)
            }
            Arguments::MkDir(mkdir) => {
                mk_dir_args(&mut tmp_buf, mkdir).unwrap();
                (NFS_PROGRAM, 9)
            }
            Arguments::SymLink(symlink) => {
                symlink_args(&mut tmp_buf, symlink).unwrap();
                (NFS_PROGRAM, 10)
            }
            Arguments::MkNod(mknod) => {
                mk_node_args(&mut tmp_buf, mknod).unwrap();
                (NFS_PROGRAM, 11)
            }
            Arguments::Remove(remove) => {
                remove_args(&mut tmp_buf, remove).unwrap();
                (NFS_PROGRAM, 12)
            }
            Arguments::RmDir(rmdir) => {
                rm_dir_args(&mut tmp_buf, rmdir).unwrap();
                (NFS_PROGRAM, 13)
            }
            Arguments::Rename(rename) => {
                rename_args(&mut tmp_buf, rename).unwrap();
                (NFS_PROGRAM, 14)
            }
            Arguments::Link(link) => {
                link_args(&mut tmp_buf, link).unwrap();
                (NFS_PROGRAM, 15)
            }
            Arguments::ReadDir(read_dir) => {
                read_dir_args(&mut tmp_buf, read_dir).unwrap();
                (NFS_PROGRAM, 16)
            }
            Arguments::ReadDirPlus(read_dir_plus) => {
                read_dir_plus_args(&mut tmp_buf, read_dir_plus).unwrap();
                (NFS_PROGRAM, 17)
            }
            Arguments::FsStat(fs_stat) => {
                fs_stat_args(&mut tmp_buf, fs_stat).unwrap();
                (NFS_PROGRAM, 18)
            }
            Arguments::FsInfo(fs_info) => {
                fs_info_args(&mut tmp_buf, fs_info).unwrap();
                (NFS_PROGRAM, 19)
            }
            Arguments::PathConf(path) => {
                path_conf_args(&mut tmp_buf, path).unwrap();
                (NFS_PROGRAM, 20)
            }
            Arguments::Commit(commit) => {
                commit_args(&mut tmp_buf, commit).unwrap();
                (NFS_PROGRAM, 21)
            }
            Arguments::Mount(mount) => {
                mount_args(&mut tmp_buf, mount).unwrap();
                (MOUNT_PROGRAM, 1)
            }
            Arguments::Unmount(unmount) => {
                unmount_args(&mut tmp_buf, unmount).unwrap();
                (MOUNT_PROGRAM, 3)
            }
            // though, apparently there is no difference between nfsv3 null and mount null
            Arguments::Null => (NFS_PROGRAM, 0),
            Arguments::Export => (MOUNT_PROGRAM, 5),
            Arguments::Dump => (MOUNT_PROGRAM, 2),
            Arguments::UnmountAll => (MOUNT_PROGRAM, 4),
        };
        let pos = tmp_buf.position() as usize;
        let size = ((tmp_buf.position() as u32) | 0x8000_0000).to_be_bytes();
        tmp_buf.set_position(0);
        tmp_buf.write(&size).unwrap();
        tmp_buf
            .write(&[0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02])
            .unwrap();
        tmp_buf.write(&prog.to_be_bytes()).unwrap();
        tmp_buf.write(&[0, 0, 0, 3]).unwrap();
        tmp_buf.write(&proc.to_be_bytes()).unwrap();
        // now we can do only Auth::None
        tmp_buf.write(&[0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        tmp_buf.set_position(0);
        self.parser.write(&tmp_buf.into_inner()[..pos]).unwrap();
    }
    pub async fn parse_message(&mut self) -> Box<Arguments> {
        self.parser.parse_message().await.unwrap()
    }
}
