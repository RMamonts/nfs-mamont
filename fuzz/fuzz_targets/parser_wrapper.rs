use arbitrary::{Arbitrary, Unstructured};
use nfs_mamont::allocator::Allocator;
use nfs_mamont::client::arguments;
use nfs_mamont::client::arguments::nfsv3::{
    access, commit, create, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node, path_conf,
    read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink, write,
};
use nfs_mamont::mocks::fuzz_socket::{FuzzMockSocket, FuzzSocketHandler};
use nfs_mamont::mount;
use nfs_mamont::nfsv3;
use nfs_mamont::nfsv3::NFS_PROGRAM;
use nfs_mamont::parser::parser_struct::RpcParser;
use nfs_mamont::parser::parser_struct::{DEFAULT_SIZE, RMS_HEADER_SIZE};
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
        let prog = u.int_in_range(NFS_PROGRAM..=mount::MOUNT_PROGRAM)?;
        let proc = u.int_in_range(0..=24)?;
        let args = match (prog, proc) {
            (NFS_PROGRAM, nfsv3::NULL) => Arguments::Null,
            (NFS_PROGRAM, nfsv3::GETATTR) => Arguments::GetAttr(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::SETATTR) => Arguments::SetAttr(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::LOOKUP) => Arguments::LookUp(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::ACCESS) => Arguments::Access(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::READLINK) => Arguments::ReadLink(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::READ) => Arguments::Read(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::WRITE) => Arguments::Write(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::CREATE) => Arguments::Create(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::MKDIR) => Arguments::MkDir(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::SYMLINK) => Arguments::SymLink(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::MKNOD) => Arguments::MkNod(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::REMOVE) => Arguments::Remove(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::RMDIR) => Arguments::RmDir(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::RENAME) => Arguments::Rename(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::LINK) => Arguments::Link(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::READDIR) => Arguments::ReadDir(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::READDIRPLUS) => Arguments::ReadDirPlus(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::FSSTAT) => Arguments::FsStat(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::FSINFO) => Arguments::FsInfo(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::PATHCONF) => Arguments::PathConf(u.arbitrary()?),
            (NFS_PROGRAM, nfsv3::COMMIT) => Arguments::Commit(u.arbitrary()?),
            (mount::MOUNT_PROGRAM, mount::NULL) => Arguments::Null,
            (mount::MOUNT_PROGRAM, mount::MOUNT) => Arguments::Mount(u.arbitrary()?),
            (mount::MOUNT_PROGRAM, mount::DUMP) => Arguments::Dump,
            (mount::MOUNT_PROGRAM, mount::UNMOUNT) => Arguments::Unmount(u.arbitrary()?),
            (mount::MOUNT_PROGRAM, mount::UNMOUNTALL) => Arguments::UnmountAll,
            (mount::MOUNT_PROGRAM, mount::EXPORT) => Arguments::Export,
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
}

impl<A: Allocator> ParserWrapper<A> {
    pub fn new(parser: RpcParser<A, FuzzMockSocket>, sender: FuzzSocketHandler) -> Self {
        // inner buffer size + max amount of bytes can be generated for Slice
        Self { parser, sender }
    }

    // forms completely new message
    pub fn write_new_message(&mut self, arg: RpcRequest) {
        let mut tmp_buffer = Vec::with_capacity(DEFAULT_SIZE + 1000);
        // tmp
        tmp_buffer.extend_from_slice(&[0, 0, 0, 0]);
        // xid
        tmp_buffer.extend_from_slice(&arg.xid.to_be_bytes());
        // call/reply
        tmp_buffer.extend_from_slice(&arg.request.to_be_bytes());
        // rpc_version
        tmp_buffer.extend_from_slice(&arg.rpc_version.to_be_bytes());
        // program
        tmp_buffer.extend_from_slice(&arg.prog.to_be_bytes());
        // program version
        tmp_buffer.extend_from_slice(&arg.version.to_be_bytes());
        // procedure
        tmp_buffer.extend_from_slice(&arg.proc.to_be_bytes());
        // now we can do only Auth::None
        tmp_buffer.extend_from_slice(&arg.auth.to_be_bytes());
        // now we can do only Auth::None
        tmp_buffer.extend_from_slice(&arg.auth_verf.to_be_bytes());
        match arg.args {
            Arguments::GetAttr(get) => {
                get_attr::get_attr_args(&mut tmp_buffer, get).unwrap();
            }
            Arguments::SetAttr(set) => {
                set_attr::set_attr_args(&mut tmp_buffer, set).unwrap();
            }
            Arguments::LookUp(lookup) => {
                lookup::lookup_args(&mut tmp_buffer, lookup).unwrap();
            }
            Arguments::Access(access) => {
                access::access_args(&mut tmp_buffer, access).unwrap();
            }
            Arguments::ReadLink(link) => {
                read_link::read_link_args(&mut tmp_buffer, link).unwrap();
            }
            Arguments::Read(read) => {
                read::read_args(&mut tmp_buffer, read).unwrap();
            }
            Arguments::Write(write) => {
                write::write_args(&mut tmp_buffer, write).unwrap();
            }
            Arguments::Create(create) => {
                create::create_args(&mut tmp_buffer, create).unwrap();
            }
            Arguments::MkDir(mkdir) => {
                mk_dir::mk_dir_args(&mut tmp_buffer, mkdir).unwrap();
            }
            Arguments::SymLink(symlink) => {
                symlink::symlink_args(&mut tmp_buffer, symlink).unwrap();
            }
            Arguments::MkNod(mknod) => {
                mk_node::mk_node_args(&mut tmp_buffer, mknod).unwrap();
            }
            Arguments::Remove(remove) => {
                remove::remove_args(&mut tmp_buffer, remove).unwrap();
            }
            Arguments::RmDir(rmdir) => {
                rm_dir::rm_dir_args(&mut tmp_buffer, rmdir).unwrap();
            }
            Arguments::Rename(rename) => {
                rename::rename_args(&mut tmp_buffer, rename).unwrap();
            }
            Arguments::Link(link) => {
                link::link_args(&mut tmp_buffer, link).unwrap();
            }
            Arguments::ReadDir(read_dir) => {
                read_dir::read_dir_args(&mut tmp_buffer, read_dir).unwrap();
            }
            Arguments::ReadDirPlus(read_dir_plus) => {
                read_dir_plus::read_dir_plus_args(&mut tmp_buffer, read_dir_plus).unwrap();
            }
            Arguments::FsStat(fs_stat) => {
                fs_stat::fs_stat_args(&mut tmp_buffer, fs_stat).unwrap();
            }
            Arguments::FsInfo(fs_info) => {
                fs_info::fs_info_args(&mut tmp_buffer, fs_info).unwrap();
            }
            Arguments::PathConf(path) => {
                path_conf::path_conf_args(&mut tmp_buffer, path).unwrap();
            }
            Arguments::Commit(commit) => {
                commit::commit_args(&mut tmp_buffer, commit).unwrap();
            }
            Arguments::Mount(mount) => {
                arguments::mount::mnt::mount_args(&mut tmp_buffer, mount).unwrap();
            }
            Arguments::Unmount(unmount) => {
                arguments::mount::unmnt::unmount_args(&mut tmp_buffer, unmount).unwrap();
            }
            // though, apparently there is no difference between nfsv3 null and mount null
            Arguments::Null | Arguments::Export | Arguments::Dump | Arguments::UnmountAll => {}
        };
        let pos = tmp_buffer.len();
        let size = ((pos - RMS_HEADER_SIZE) as u32 | 0x8000_0000).to_be_bytes();
        tmp_buffer[..RMS_HEADER_SIZE].copy_from_slice(size.as_slice());
        // there should be sending to mpsc
        self.sender.send_data(tmp_buffer);
    }
    pub async fn parse_message(&mut self) -> Result<Box<Arguments>> {
        self.parser.parse_message().await
    }
}
