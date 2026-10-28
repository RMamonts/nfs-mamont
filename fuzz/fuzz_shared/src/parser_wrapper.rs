use arbitrary::{Arbitrary, Unstructured};

use nfs_mamont::{
    arguments, consts, ArgWrapper, ErrorWrapper, MockAllocator, MockBuffers, MountArguments,
    NfsArguments, NlmArguments, ProcArguments, RpcBody, RpcParser, DEFAULT_SIZE, MOUNT_PROGRAM,
    MOUNT_VERSION, NFS_PROGRAM, NFS_VERSION, RMS_HEADER_SIZE, RPC_VERSION,
};

use nfs_mamont::arguments::nfsv3::{
    access, commit, create, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node, path_conf,
    read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink, write,
};
use nfs_mamont::consts::mount::MountProc;
use nfs_mamont::consts::nfsv3::NfsProc;
use nfs_mamont::consts::nlm::{NlmProc, NLM_PROGRAM};

use crate::read_socket::{FuzzMockSocket, FuzzSocketHandler};

type TestParser = RpcParser<MockAllocator, FuzzMockSocket>;

#[derive(Clone, Debug)]
pub struct RpcRequest {
    pub xid: u32,
    pub request: u32,
    pub rpc_version: u32,
    pub prog: u32,
    pub version: u32,
    pub proc: u32,
    // for now only None (0)
    pub auth: u32,
    // for now only None (0)
    pub auth_verf: u32,
    pub args: ProcArguments<MockBuffers>,
}

impl<'a> Arbitrary<'a> for RpcRequest {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let prog = *u.choose(&[MOUNT_PROGRAM, NFS_PROGRAM, NLM_PROGRAM])?;
        let (args, proc) = match prog {
            NFS_PROGRAM => match NfsProc::arbitrary(u)? {
                NfsProc::Null => {
                    (ProcArguments::Nfs3(Box::new(NfsArguments::Null)), NfsProc::Null as u32)
                }
                NfsProc::GetAttr => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::GetAttr(u.arbitrary()?))),
                    NfsProc::GetAttr as u32,
                ),
                NfsProc::SetAttr => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::SetAttr(u.arbitrary()?))),
                    NfsProc::SetAttr as u32,
                ),
                NfsProc::LookUp => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::LookUp(u.arbitrary()?))),
                    NfsProc::LookUp as u32,
                ),
                NfsProc::Access => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::Access(u.arbitrary()?))),
                    NfsProc::Access as u32,
                ),
                NfsProc::ReadLink => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::ReadLink(u.arbitrary()?))),
                    NfsProc::ReadLink as u32,
                ),
                NfsProc::Read => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::Read(
                        u.arbitrary()?,
                        u.arbitrary()?,
                    ))),
                    NfsProc::Read as u32,
                ),
                NfsProc::Write => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::Write(u.arbitrary()?))),
                    NfsProc::Write as u32,
                ),
                NfsProc::Create => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::Create(u.arbitrary()?))),
                    NfsProc::Create as u32,
                ),
                NfsProc::MkDir => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::MkDir(u.arbitrary()?))),
                    NfsProc::MkDir as u32,
                ),
                NfsProc::SymLink => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::SymLink(u.arbitrary()?))),
                    NfsProc::SymLink as u32,
                ),
                NfsProc::MkNod => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::MkNod(u.arbitrary()?))),
                    NfsProc::MkNod as u32,
                ),
                NfsProc::Remove => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::Remove(u.arbitrary()?))),
                    NfsProc::Remove as u32,
                ),
                NfsProc::RmDir => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::RmDir(u.arbitrary()?))),
                    NfsProc::RmDir as u32,
                ),
                NfsProc::Rename => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::Rename(u.arbitrary()?))),
                    NfsProc::Rename as u32,
                ),
                NfsProc::Link => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::Link(u.arbitrary()?))),
                    NfsProc::Link as u32,
                ),
                NfsProc::ReadDir => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::ReadDir(u.arbitrary()?))),
                    NfsProc::ReadDir as u32,
                ),
                NfsProc::ReadDirPlus => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::ReadDirPlus(u.arbitrary()?))),
                    NfsProc::ReadDirPlus as u32,
                ),
                NfsProc::FsStat => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::FsStat(u.arbitrary()?))),
                    NfsProc::FsStat as u32,
                ),
                NfsProc::FsInfo => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::FsInfo(u.arbitrary()?))),
                    NfsProc::FsInfo as u32,
                ),
                NfsProc::PathConf => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::PathConf(u.arbitrary()?))),
                    NfsProc::PathConf as u32,
                ),
                NfsProc::Commit => (
                    ProcArguments::Nfs3(Box::new(NfsArguments::Commit(u.arbitrary()?))),
                    NfsProc::Commit as u32,
                ),
            },
            MOUNT_PROGRAM => match consts::mount::MountProc::arbitrary(u)? {
                MountProc::Null => {
                    (ProcArguments::Mount(Box::new(MountArguments::Null)), MountProc::Null as u32)
                }
                MountProc::Mnt => (
                    ProcArguments::Mount(Box::new(MountArguments::Mount(u.arbitrary()?))),
                    MountProc::Mnt as u32,
                ),
                MountProc::Dump => {
                    (ProcArguments::Mount(Box::new(MountArguments::Dump)), MountProc::Dump as u32)
                }
                MountProc::Umnt => (
                    ProcArguments::Mount(Box::new(MountArguments::Unmount(u.arbitrary()?))),
                    MountProc::Umnt as u32,
                ),
                MountProc::UmntAll => (
                    ProcArguments::Mount(Box::new(MountArguments::UnmountAll)),
                    MountProc::UmntAll as u32,
                ),
                MountProc::Export => (
                    ProcArguments::Mount(Box::new(MountArguments::Export)),
                    MountProc::Export as u32,
                ),
            },
            NLM_PROGRAM => match NlmProc::arbitrary(u)? {
                NlmProc::Null => {
                    (ProcArguments::Nlm4(Box::new(NlmArguments::Null)), NlmProc::Null as u32)
                }
                NlmProc::Test => (
                    ProcArguments::Nlm4(Box::new(NlmArguments::Test(u.arbitrary()?))),
                    NlmProc::Test as u32,
                ),
                NlmProc::Lock => (
                    ProcArguments::Nlm4(Box::new(NlmArguments::Lock(u.arbitrary()?))),
                    NlmProc::Lock as u32,
                ),
                NlmProc::Cancel => (
                    ProcArguments::Nlm4(Box::new(NlmArguments::Cancel(u.arbitrary()?))),
                    NlmProc::Cancel as u32,
                ),
                NlmProc::Unlock => (
                    ProcArguments::Nlm4(Box::new(NlmArguments::Unlock(u.arbitrary()?))),
                    NlmProc::Unlock as u32,
                ),
            },
            _ => unreachable!(),
        };
        Ok(Self {
            xid: u.arbitrary()?,
            request: *u.choose(&[RpcBody::Call as u32, RpcBody::Reply as u32])?,
            //so there would be RpcVersionMismatch
            rpc_version: *u.choose(&[RPC_VERSION])?,
            //so there would be ProgramMismatch
            prog,
            //so there would be ProgramVersionMismatch
            version: *u.choose(&[MOUNT_VERSION, NFS_VERSION])?,
            //so there would be ProcedureMismatch (nfsv3 has 21 proc)
            proc,
            auth: 0,
            auth_verf: 0,
            args,
        })
    }
}

pub struct ParserWrapper {
    parser: TestParser,
    sender: FuzzSocketHandler,
}

impl ParserWrapper {
    pub fn new(parser: TestParser, sender: FuzzSocketHandler) -> Self {
        Self { parser, sender }
    }

    // forms completely new message
    pub fn write_new_message(&mut self, arg: RpcRequest) {
        let mut tmp_buffer = Vec::with_capacity(DEFAULT_SIZE);
        // place for size
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
        // now we can do only Auth::None
        tmp_buffer.extend_from_slice(&arg.auth.to_be_bytes());
        // now we can do only Auth::None
        tmp_buffer.extend_from_slice(&arg.auth_verf.to_be_bytes());
        match arg.args {
            ProcArguments::Nfs3(nfs) => Self::match_nfsv3(&mut tmp_buffer, *nfs),
            ProcArguments::Mount(mnt) => Self::match_mount(&mut tmp_buffer, *mnt),
            ProcArguments::Nlm4(nlm) => Self::match_nlm(&mut tmp_buffer, *nlm),
        }
        let pos = tmp_buffer.len();
        assert!(pos - RMS_HEADER_SIZE < 0x8000_0000);
        let size = ((pos - RMS_HEADER_SIZE) as u32 | 0x8000_0000).to_be_bytes();
        tmp_buffer[..RMS_HEADER_SIZE].copy_from_slice(size.as_slice());
        // there should be sending to mpsc
        assert_eq!(pos, tmp_buffer.len());
        self.sender.send_data(tmp_buffer);
    }
    pub async fn parse_message(&mut self) -> Result<ArgWrapper<MockBuffers>, ErrorWrapper> {
        self.parser.next_message().await
    }

    fn match_nfsv3(tmp_buffer: &mut Vec<u8>, nfs: NfsArguments<MockBuffers>) {
        match nfs {
            NfsArguments::GetAttr(get) => get_attr::get_attr_args(tmp_buffer, get).unwrap(),

            NfsArguments::SetAttr(set) => set_attr::set_attr_args(tmp_buffer, set).unwrap(),

            NfsArguments::LookUp(lookup) => lookup::lookup_args(tmp_buffer, lookup).unwrap(),

            NfsArguments::Access(access) => access::access_args(tmp_buffer, access).unwrap(),

            NfsArguments::ReadLink(link) => read_link::read_link_args(tmp_buffer, link).unwrap(),

            NfsArguments::Read(read, ..) => read::read_args(tmp_buffer, read).unwrap(),

            NfsArguments::Write(write) => write::write_args(tmp_buffer, write).unwrap(),

            NfsArguments::Create(create) => create::create_args(tmp_buffer, create).unwrap(),

            NfsArguments::MkDir(mkdir) => mk_dir::mk_dir_args(tmp_buffer, mkdir).unwrap(),

            NfsArguments::SymLink(symlink) => symlink::symlink_args(tmp_buffer, symlink).unwrap(),

            NfsArguments::MkNod(mknod) => mk_node::mk_node_args(tmp_buffer, mknod).unwrap(),

            NfsArguments::Remove(remove) => remove::remove_args(tmp_buffer, remove).unwrap(),

            NfsArguments::RmDir(rmdir) => rm_dir::rm_dir_args(tmp_buffer, rmdir).unwrap(),

            NfsArguments::Rename(rename) => rename::rename_args(tmp_buffer, rename).unwrap(),

            NfsArguments::Link(link) => link::link_args(tmp_buffer, link).unwrap(),

            NfsArguments::ReadDir(read_dir) => {
                read_dir::read_dir_args(tmp_buffer, read_dir).unwrap()
            }

            NfsArguments::ReadDirPlus(read_dir_plus) => {
                read_dir_plus::read_dir_plus_args(tmp_buffer, read_dir_plus).unwrap()
            }

            NfsArguments::FsStat(fs_stat) => fs_stat::fs_stat_args(tmp_buffer, fs_stat).unwrap(),

            NfsArguments::FsInfo(fs_info) => fs_info::fs_info_args(tmp_buffer, fs_info).unwrap(),

            NfsArguments::PathConf(path) => path_conf::path_conf_args(tmp_buffer, path).unwrap(),

            NfsArguments::Commit(commit) => commit::commit_args(tmp_buffer, commit).unwrap(),

            NfsArguments::Null => (),
        }
    }

    fn match_mount(tmp_buffer: &mut Vec<u8>, mnt: MountArguments) {
        match mnt {
            MountArguments::Mount(mount) => {
                arguments::mount::mnt::mount_args(tmp_buffer, mount).unwrap()
            }
            MountArguments::Unmount(unmount) => {
                arguments::mount::unmnt::unmount_args(tmp_buffer, unmount).unwrap()
            }

            MountArguments::Export => (),
            MountArguments::Dump => (),
            MountArguments::UnmountAll => (),
            MountArguments::Null => (),
        }
    }

    fn match_nlm(tmp_buffer: &mut Vec<u8>, nlm: NlmArguments) {
        match nlm {
            NlmArguments::Cancel(cancel) => {
                arguments::nlm4::cancel::cancel_args(tmp_buffer, cancel).unwrap()
            }
            NlmArguments::Test(test) => arguments::nlm4::test::test_args(tmp_buffer, test).unwrap(),
            NlmArguments::Lock(lock) => arguments::nlm4::lock::lock_args(tmp_buffer, lock).unwrap(),
            NlmArguments::Unlock(unlock) => {
                arguments::nlm4::unlock::unlock_args(tmp_buffer, unlock).unwrap()
            }
            NlmArguments::Null => (),
        }
    }
}
