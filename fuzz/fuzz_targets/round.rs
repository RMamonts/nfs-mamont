#![no_main]

use std::io::Cursor;

use libfuzzer_sys::fuzz_target;
use nfs_mamont::client::arguments;
use nfs_mamont::parser::primitive::{u32_as_usize, ALIGNMENT};
use nfs_mamont::parser::{mount, nfsv3, Arguments};

const DEFAULT_CAPACITY: usize =
    nfs_mamont::parser::parser_struct::DEFAULT_SIZE + nfs_mamont::allocator::TEST_SIZE;

macro_rules! roundtrip {
    ($arg:expr, $write:path, $read:path) => {{
        let mut buf = Cursor::new(vec![0u8; DEFAULT_CAPACITY]);
        $write(&mut buf, $arg.clone()).unwrap();
        let len = buf.position();
        buf.set_position(0);
        assert_eq!($read(&mut buf).unwrap(), $arg);
        assert_eq!(buf.position(), len);
    }};
}

fuzz_target!(|data: Arguments| {
    match data {
        Arguments::GetAttr(arg) => {
            roundtrip!(arg, arguments::nfsv3::get_attr::get_attr_args, nfsv3::get_attr::args)
        }
        Arguments::SetAttr(arg) => {
            roundtrip!(arg, arguments::nfsv3::set_attr::set_attr_args, nfsv3::set_attr::args)
        }
        Arguments::LookUp(arg) => {
            roundtrip!(arg, arguments::nfsv3::lookup::lookup_args, nfsv3::lookup::args)
        }
        Arguments::Access(arg) => {
            roundtrip!(arg, arguments::nfsv3::access::access_args, nfsv3::access::args)
        }
        Arguments::ReadLink(arg) => {
            roundtrip!(arg, arguments::nfsv3::read_link::read_link_args, nfsv3::read_link::args)
        }
        Arguments::Read(arg) => {
            roundtrip!(arg, arguments::nfsv3::read::read_args, nfsv3::read::args)
        }
        Arguments::Create(arg) => {
            roundtrip!(arg, arguments::nfsv3::create::create_args, nfsv3::create::args)
        }
        Arguments::MkDir(arg) => {
            roundtrip!(arg, arguments::nfsv3::mk_dir::mk_dir_args, nfsv3::mk_dir::args)
        }
        Arguments::SymLink(arg) => {
            roundtrip!(arg, arguments::nfsv3::symlink::symlink_args, nfsv3::symlink::args)
        }
        Arguments::MkNod(arg) => {
            roundtrip!(arg, arguments::nfsv3::mk_node::mk_node_args, nfsv3::mk_node::args)
        }
        Arguments::Remove(arg) => {
            roundtrip!(arg, arguments::nfsv3::remove::remove_args, nfsv3::remove::args)
        }
        Arguments::RmDir(arg) => {
            roundtrip!(arg, arguments::nfsv3::rm_dir::rm_dir_args, nfsv3::rm_dir::args)
        }
        Arguments::Rename(arg) => {
            roundtrip!(arg, arguments::nfsv3::rename::rename_args, nfsv3::rename::args)
        }
        Arguments::Link(arg) => {
            roundtrip!(arg, arguments::nfsv3::link::link_args, nfsv3::link::args)
        }
        Arguments::ReadDir(arg) => {
            roundtrip!(arg, arguments::nfsv3::read_dir::read_dir_args, nfsv3::read_dir::args)
        }
        Arguments::ReadDirPlus(arg) => roundtrip!(
            arg,
            arguments::nfsv3::read_dir_plus::read_dir_plus_args,
            nfsv3::read_dir_plus::args
        ),
        Arguments::FsStat(arg) => {
            roundtrip!(arg, arguments::nfsv3::fs_stat::fs_stat_args, nfsv3::fs_stat::args)
        }
        Arguments::FsInfo(arg) => {
            roundtrip!(arg, arguments::nfsv3::fs_info::fs_info_args, nfsv3::fs_info::args)
        }
        Arguments::PathConf(arg) => {
            roundtrip!(arg, arguments::nfsv3::path_conf::path_conf_args, nfsv3::path_conf::args)
        }
        Arguments::Commit(arg) => {
            roundtrip!(arg, arguments::nfsv3::commit::commit_args, nfsv3::commit::args)
        }
        Arguments::Mount(arg) => {
            roundtrip!(arg, arguments::mount::mnt::mount_args, mount::mnt::mount)
        }
        Arguments::Unmount(arg) => {
            roundtrip!(arg, arguments::mount::unmnt::unmount_args, mount::umnt::unmount)
        }
        Arguments::Write(args) => {
            let mut buf = Cursor::new(vec![0u8; DEFAULT_CAPACITY]);
            arguments::nfsv3::write::write_args(&mut buf, args.clone()).unwrap();
            let len = buf.position();
            buf.set_position(0);
            let patrial = nfsv3::write::args(&mut buf).unwrap();
            let size = u32_as_usize(&mut buf).unwrap();
            let pos = buf.position() as usize;
            let opaque = buf.into_inner();
            let mut read = pos;
            for block in args.data.iter() {
                let size = block.len();
                assert_eq!(*block, opaque[read..read + size]);
                read += size;
            }
            assert_eq!(patrial.size, args.size);
            assert_eq!(patrial.file, args.file);
            assert_eq!(patrial.offset, args.offset);
            assert_eq!(patrial.stable, args.stable);
            let padding = (ALIGNMENT - size % ALIGNMENT) % ALIGNMENT;
            assert_eq!(pos + size + padding, len as usize)
        }
        _ => {}
    };
});
