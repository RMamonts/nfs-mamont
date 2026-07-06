use arbitrary::{Arbitrary, Unstructured};

use nfs_mamont::{
    arguments, nfsv3, parser_mount, parser_nlm, ArgWrapper, ErrorWrapper, MockAllocator,
    MockBuffers, MountArguments, NfsArguments, NlmArguments, ProcArguments, RpcBody, RpcParser,
    ACCESS, COMMIT, CREATE, DEFAULT_SIZE, FSINFO, FSSTAT, GETATTR, LINK, LOOKUP, MKDIR, MKNOD,
    MOUNT_DUMP, MOUNT_EXPORT, MOUNT_MNT, MOUNT_NULL, MOUNT_PROGRAM, MOUNT_UMNT, MOUNT_UMNTALL,
    MOUNT_VERSION, NFS_PROGRAM, NFS_VERSION, NULL, PATHCONF, READ, READDIR, READDIRPLUS, READLINK,
    REMOVE, RENAME, RMDIR, RMS_HEADER_SIZE, RPC_VERSION, SETATTR, SYMLINK, TEST_SIZE, WRITE,
};

// Re-export serializer functions
use nfs_mamont::arguments::nfsv3::{
    access, commit, create, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node, path_conf,
    read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink, write,
};

use crate::read_socket::{FuzzMockSocket, FuzzSocketHandler};

type TestParser = RpcParser<MockAllocator, FuzzMockSocket>;
const FAULT_VERSION: u32 = 7;
const FAULT_PROGRAM: u32 = 1;

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
        let prog = *u.choose(&[MOUNT_PROGRAM, NFS_PROGRAM, FAULT_PROGRAM])?;
        let proc = match prog {
            NFS_PROGRAM => u.int_in_range(0..=22)?,
            MOUNT_PROGRAM => u.int_in_range(0..=6)?,
            FAULT_PROGRAM => u.int_in_range(0..=22)?,
            _ => u.int_in_range(0..=22)?,
        };
        let args = match (prog, proc) {
            (NFS_PROGRAM, NULL) => ProcArguments::Nfs3(Box::new(NfsArguments::Null)),
            (NFS_PROGRAM, GETATTR) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::GetAttr(u.arbitrary()?)))
            }
            (NFS_PROGRAM, SETATTR) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::SetAttr(u.arbitrary()?)))
            }
            (NFS_PROGRAM, LOOKUP) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::LookUp(u.arbitrary()?)))
            }
            (NFS_PROGRAM, ACCESS) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::Access(u.arbitrary()?)))
            }
            (NFS_PROGRAM, READLINK) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::ReadLink(u.arbitrary()?)))
            }
            (NFS_PROGRAM, READ) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::Read(u.arbitrary()?)))
            }
            (NFS_PROGRAM, WRITE) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::Write(u.arbitrary()?)))
            }
            (NFS_PROGRAM, CREATE) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::Create(u.arbitrary()?)))
            }
            (NFS_PROGRAM, MKDIR) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::MkDir(u.arbitrary()?)))
            }
            (NFS_PROGRAM, SYMLINK) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::SymLink(u.arbitrary()?)))
            }
            (NFS_PROGRAM, MKNOD) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::MkNod(u.arbitrary()?)))
            }
            (NFS_PROGRAM, REMOVE) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::Remove(u.arbitrary()?)))
            }
            (NFS_PROGRAM, RMDIR) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::RmDir(u.arbitrary()?)))
            }
            (NFS_PROGRAM, RENAME) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::Rename(u.arbitrary()?)))
            }
            (NFS_PROGRAM, LINK) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::Link(u.arbitrary()?)))
            }
            (NFS_PROGRAM, READDIR) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::ReadDir(u.arbitrary()?)))
            }
            (NFS_PROGRAM, READDIRPLUS) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::ReadDirPlus(u.arbitrary()?)))
            }
            (NFS_PROGRAM, FSSTAT) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::FsStat(u.arbitrary()?)))
            }
            (NFS_PROGRAM, FSINFO) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::FsInfo(u.arbitrary()?)))
            }
            (NFS_PROGRAM, PATHCONF) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::PathConf(u.arbitrary()?)))
            }
            (NFS_PROGRAM, COMMIT) => {
                ProcArguments::Nfs3(Box::new(NfsArguments::Commit(u.arbitrary()?)))
            }
            (MOUNT_PROGRAM, MOUNT_NULL) => ProcArguments::Mount(Box::new(MountArguments::Null)),
            (MOUNT_PROGRAM, MOUNT_MNT) => {
                ProcArguments::Mount(Box::new(MountArguments::Mount(u.arbitrary()?)))
            }
            (MOUNT_PROGRAM, MOUNT_DUMP) => ProcArguments::Mount(Box::new(MountArguments::Dump)),
            (MOUNT_PROGRAM, MOUNT_UMNT) => {
                ProcArguments::Mount(Box::new(MountArguments::Unmount(u.arbitrary()?)))
            }
            (MOUNT_PROGRAM, MOUNT_UMNTALL) => {
                ProcArguments::Mount(Box::new(MountArguments::UnmountAll))
            }
            (MOUNT_PROGRAM, MOUNT_EXPORT) => ProcArguments::Mount(Box::new(MountArguments::Export)),
            _ => u.arbitrary::<ProcArguments<MockBuffers>>()?,
        };
        Ok(Self {
            xid: u.arbitrary()?,
            request: *u.choose(&[RpcBody::Call as u32, RpcBody::Reply as u32])?,
            //so there would be RpcVersionMismatch
            rpc_version: *u.choose(&[RPC_VERSION, FAULT_VERSION])?,
            //so there would be ProgramMismatch
            prog,
            //so there would be ProgramVersionMismatch
            version: *u.choose(&[MOUNT_VERSION, NFS_VERSION, FAULT_VERSION])?,
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
        let mut tmp_buffer = Vec::with_capacity(DEFAULT_SIZE + TEST_SIZE);
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
            ProcArguments::Nfs3(nfs) => match *nfs {
                NfsArguments::GetAttr(get) => {
                    get_attr::get_attr_args(&mut tmp_buffer, get).unwrap()
                }

                NfsArguments::SetAttr(set) => {
                    set_attr::set_attr_args(&mut tmp_buffer, set).unwrap()
                }

                NfsArguments::LookUp(lookup) => {
                    lookup::lookup_args(&mut tmp_buffer, lookup).unwrap()
                }

                NfsArguments::Access(access) => {
                    access::access_args(&mut tmp_buffer, access).unwrap()
                }

                NfsArguments::ReadLink(link) => {
                    read_link::read_link_args(&mut tmp_buffer, link).unwrap()
                }

                NfsArguments::Read(read) => read::read_args(&mut tmp_buffer, read).unwrap(),

                NfsArguments::Write(write) => write::write_args(&mut tmp_buffer, write).unwrap(),

                NfsArguments::Create(create) => {
                    create::create_args(&mut tmp_buffer, create).unwrap()
                }

                NfsArguments::MkDir(mkdir) => mk_dir::mk_dir_args(&mut tmp_buffer, mkdir).unwrap(),

                NfsArguments::SymLink(symlink) => {
                    symlink::symlink_args(&mut tmp_buffer, symlink).unwrap()
                }

                NfsArguments::MkNod(mknod) => {
                    mk_node::mk_node_args(&mut tmp_buffer, mknod).unwrap()
                }

                NfsArguments::Remove(remove) => {
                    remove::remove_args(&mut tmp_buffer, remove).unwrap()
                }

                NfsArguments::RmDir(rmdir) => rm_dir::rm_dir_args(&mut tmp_buffer, rmdir).unwrap(),

                NfsArguments::Rename(rename) => {
                    rename::rename_args(&mut tmp_buffer, rename).unwrap()
                }

                NfsArguments::Link(link) => link::link_args(&mut tmp_buffer, link).unwrap(),

                NfsArguments::ReadDir(read_dir) => {
                    read_dir::read_dir_args(&mut tmp_buffer, read_dir).unwrap()
                }

                NfsArguments::ReadDirPlus(read_dir_plus) => {
                    read_dir_plus::read_dir_plus_args(&mut tmp_buffer, read_dir_plus).unwrap()
                }

                NfsArguments::FsStat(fs_stat) => {
                    fs_stat::fs_stat_args(&mut tmp_buffer, fs_stat).unwrap()
                }

                NfsArguments::FsInfo(fs_info) => {
                    fs_info::fs_info_args(&mut tmp_buffer, fs_info).unwrap()
                }

                NfsArguments::PathConf(path) => {
                    path_conf::path_conf_args(&mut tmp_buffer, path).unwrap()
                }

                NfsArguments::Commit(commit) => {
                    commit::commit_args(&mut tmp_buffer, commit).unwrap()
                }

                NfsArguments::Null => (),
            },

            ProcArguments::Mount(mnt) => match *mnt {
                MountArguments::Mount(mount) => {
                    arguments::mount::mnt::mount_args(&mut tmp_buffer, mount).unwrap()
                }

                MountArguments::Unmount(unmount) => {
                    arguments::mount::unmnt::unmount_args(&mut tmp_buffer, unmount).unwrap()
                }

                MountArguments::Export => (),
                MountArguments::Dump => (),
                MountArguments::UnmountAll => (),
                MountArguments::Null => (),
            },
            ProcArguments::Nlm4(nlm) => match *nlm {
                NlmArguments::Cancel(cancel) => {
                    arguments::nlm4::cancel::cancel_args(&mut tmp_buffer, cancel).unwrap()
                }
                NlmArguments::Test(test) => {
                    arguments::nlm4::test::test_args(&mut tmp_buffer, test).unwrap()
                }
                NlmArguments::Lock(lock) => {
                    arguments::nlm4::lock::lock_args(&mut tmp_buffer, lock).unwrap()
                }
                NlmArguments::Unlock(unlock) => {
                    arguments::nlm4::unlock::unlock_args(&mut tmp_buffer, unlock).unwrap()
                }
                NlmArguments::Null => (),
            },
        }

        let pos = tmp_buffer.len();
        assert!(pos - RMS_HEADER_SIZE < 0x8000_0000);
        let size = ((pos - RMS_HEADER_SIZE) as u32 | 0x8000_0000).to_be_bytes();
        tmp_buffer[..RMS_HEADER_SIZE].copy_from_slice(size.as_slice());
        // there should be sending to mpsc
        self.sender.send_data(tmp_buffer);
    }
    pub async fn parse_message(&mut self) -> Result<ArgWrapper<MockBuffers>, ErrorWrapper> {
        self.parser.next_message().await
    }
}
