#![no_main]

use libfuzzer_sys::fuzz_target;
use nfs_mamont::client::arguments;
use nfs_mamont::parser::{mount, nfsv3, Arguments};
use std::io::Cursor;

fuzz_target!(|data: Arguments| {
    let mut buf = Cursor::new(vec![0u8; 6500]);
    match data {
        Arguments::GetAttr(arg) => {
            arguments::get_attr::get_attr_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::get_attr::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::SetAttr(arg) => {
            arguments::set_attr::set_attr_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::set_attr::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::LookUp(arg) => {
            arguments::lookup::lookup_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::lookup::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::Access(arg) => {
            arguments::access::access_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::access::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::ReadLink(arg) => {
            arguments::read_link::read_link_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::read_link::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::Read(arg) => {
            arguments::read::read_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::read::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::Write(_) => {}
        Arguments::Create(arg) => {
            arguments::create::create_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::create::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::MkDir(arg) => {
            arguments::mk_dir::mk_dir_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::mk_dir::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::SymLink(arg) => {
            arguments::symlink::symlink_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::symlink::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::MkNod(arg) => {
            arguments::mk_node::mk_node_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::mk_node::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::Remove(arg) => {
            arguments::remove::remove_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::remove::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::RmDir(arg) => {
            arguments::rm_dir::rm_dir_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::rm_dir::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::Rename(arg) => {
            arguments::rename::rename_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::rename::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::Link(arg) => {
            arguments::link::link_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::link::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::ReadDir(arg) => {
            arguments::read_dir::read_dir_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::read_dir::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::ReadDirPlus(arg) => {
            arguments::read_dir_plus::read_dir_plus_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::read_dir_plus::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::FsStat(arg) => {
            arguments::fs_stat::fs_stat_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::fs_stat::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::FsInfo(arg) => {
            arguments::fs_info::fs_info_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::fs_info::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::PathConf(arg) => {
            arguments::path_conf::path_conf_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::path_conf::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::Commit(arg) => {
            arguments::commit::commit_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = nfsv3::commit::args(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::Mount(arg) => {
            arguments::mount::mount_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = mount::mnt::mount(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        Arguments::Unmount(arg) => {
            arguments::mount::unmount_args(&mut buf, arg.clone()).unwrap();
            buf.set_position(0);
            let res = mount::umnt::unmount(&mut buf).unwrap();
            assert_eq!(res, arg)
        }
        _ => {}
    }
});
