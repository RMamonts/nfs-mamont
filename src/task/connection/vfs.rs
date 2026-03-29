use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tracing::error;

use crate::allocator::{Allocator, Impl, Slice};
use crate::context::{HandleMap, ServerContext};
use crate::parser::{NfsArgWrapper, NfsArguments};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::{self, NfsRes, Vfs, WccData};

/// Process RPC commands, sends operation results to [`crate::task::connection::write::WriteTask`].
pub struct VfsTask {
    backend: Arc<dyn Vfs + Send + Sync + 'static>,
    allocator: Arc<Mutex<Impl>>,
    handles: Arc<HandleMap>,
    command_receiver: UnboundedReceiver<NfsArgWrapper>,
    result_sender: UnboundedSender<ProcReply>,
}

impl VfsTask {
    /// Creates new instance of [`VfsTask`].
    pub fn new(
        context: &ServerContext,
        command_receiver: UnboundedReceiver<NfsArgWrapper>,
        result_sender: UnboundedSender<ProcReply>,
    ) -> Self {
        Self {
            backend: context.get_backend(),
            allocator: context.get_read_allocator(),
            handles: context.get_handle_map(),
            command_receiver,
            result_sender,
        }
    }

    /// Spawns a [`VfsTask`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let mut command_receiver = self.command_receiver;

        while let Some(command) = command_receiver.recv().await {
            let NfsArgWrapper { header, proc } = command;
            let proc_name = Self::proc_name(&proc);

            let response = match *proc {
                NfsArguments::Null => NfsRes::Null,
                NfsArguments::GetAttr(args) => match self.handles.path_for_handle(&args.file) {
                    Ok(path) => NfsRes::GetAttr(self.backend.get_attr(args, path).await),
                    Err(error) => NfsRes::GetAttr(Err(vfs::get_attr::Fail { error })),
                },
                NfsArguments::SetAttr(args) => match self.handles.path_for_handle(&args.file) {
                    Ok(path) => NfsRes::SetAttr(self.backend.set_attr(args, path).await),
                    Err(error) => NfsRes::SetAttr(Err(vfs::set_attr::Fail {
                        error,
                        wcc_data: WccData { before: None, after: None },
                    })),
                },

                NfsArguments::LookUp(args) => match self.handles.path_for_handle(&args.parent) {
                    Ok(path) => NfsRes::LookUp(self.backend.lookup(args, path).await),
                    Err(error) => NfsRes::LookUp(Err(vfs::lookup::Fail { error, dir_attr: None })),
                },
                NfsArguments::Access(args) => match self.handles.path_for_handle(&args.file) {
                    Ok(path) => NfsRes::Access(self.backend.access(args, path).await),
                    Err(error) => {
                        NfsRes::Access(Err(vfs::access::Fail { error, object_attr: None }))
                    }
                },

                NfsArguments::ReadLink(args) => match self.handles.path_for_handle(&args.file) {
                    Ok(path) => NfsRes::ReadLink(self.backend.read_link(args, path).await),
                    Err(error) => {
                        NfsRes::ReadLink(Err(vfs::read_link::Fail { symlink_attr: None, error }))
                    }
                },
                NfsArguments::Read(args) => match self.handles.path_for_handle(&args.file) {
                    Ok(path) => {
                        let data_result = if args.count == 0 {
                            Ok(Slice::empty())
                        } else {
                            let requested_size = NonZeroUsize::new(args.count as usize).unwrap();

                            let mut allocator = self.allocator.lock().await;
                            allocator.allocate(requested_size).await.ok_or(vfs::read::Fail {
                                error: vfs::Error::TooSmall,
                                file_attr: None,
                            })
                        };

                        match data_result {
                            Ok(data) => NfsRes::Read(self.backend.read(args, data, path).await),
                            Err(err) => NfsRes::Read(Err(err)),
                        }
                    }
                    Err(error) => NfsRes::Read(vfs::read::Fail { error, file_attr: None }),
                },
                NfsArguments::Write(args) => match self.handles.path_for_handle(&args.file) {
                    Ok(path) => NfsRes::Write(self.backend.write(args, path).await),
                    Err(error) => NfsRes::Write(Err(vfs::write::Fail {
                        error,
                        wcc_data: WccData { before: None, after: None },
                    })),
                },
                NfsArguments::Create(args) => {
                    match self.handles.path_for_handle(&args.object.dir) {
                        Ok(path) => NfsRes::Create(self.backend.create(args, path).await),
                        Err(error) => NfsRes::Create(Err(vfs::create::Fail {
                            error,
                            wcc_data: WccData { before: None, after: None },
                        })),
                    }
                }

                NfsArguments::MkDir(args) => match self.handles.path_for_handle(&args.object.dir) {
                    Ok(path) => NfsRes::MkDir(self.backend.mk_dir(args, path).await),
                    Err(error) => NfsRes::MkDir(Err(vfs::mk_dir::Fail {
                        error,
                        dir_wcc: WccData { before: None, after: None },
                    })),
                },
                NfsArguments::SymLink(args) => match self.handles.path_for_handle(&args.object.dir) {
                    Ok(path) => NfsRes::SymLink(self.backend.symlink(args, path).await),
                    Err(error) => NfsRes::MkDir(Err(vfs::mk_dir::Fail {
                        error,
                        dir_wcc: WccData { before: None, after: None },
                    })),
                },


                NfsArguments::MkNod(args) => NfsRes::MkNod(self.backend.mk_node(args).await),
                NfsArguments::Remove(args) => NfsRes::Remove(self.backend.remove(args).await),
                NfsArguments::RmDir(args) => NfsRes::RmDir(self.backend.rm_dir(args).await),
                NfsArguments::Rename(args) => NfsRes::Rename(self.backend.rename(args).await),
                NfsArguments::Link(args) => NfsRes::Link(self.backend.link(args).await),
                NfsArguments::ReadDir(args) => NfsRes::ReadDir(self.backend.read_dir(args).await),
                NfsArguments::ReadDirPlus(args) => {
                    NfsRes::ReadDirPlus(self.backend.read_dir_plus(args).await)
                }
                NfsArguments::FsStat(args) => NfsRes::FsStat(self.backend.fs_stat(args).await),
                NfsArguments::FsInfo(args) => NfsRes::FsInfo(self.backend.fs_info(args).await),
                NfsArguments::PathConf(args) => {
                    NfsRes::PathConf(self.backend.path_conf(args).await)
                }
                NfsArguments::Commit(args) => NfsRes::Commit(self.backend.commit(args).await),
            };

            if let Some(error) = Self::error_from_response(&response) {
                error!(xid=header.xid, proc=%proc_name, error=?error, "nfs op failed");
            }

            let reply = ProcReply {
                xid: header.xid,
                proc_result: Ok(ProcResult::Nfs3(Box::new(response))),
            };

            // Write task may already be closed; then this connection pipeline is done.
            if self.result_sender.send(reply).is_err() {
                return;
            }
        }
    }

    fn proc_name(proc: &NfsArguments) -> &'static str {
        match proc {
            NfsArguments::Null => "NULL",
            NfsArguments::GetAttr(_) => "GETATTR",
            NfsArguments::SetAttr(_) => "SETATTR",
            NfsArguments::LookUp(_) => "LOOKUP",
            NfsArguments::Access(_) => "ACCESS",
            NfsArguments::ReadLink(_) => "READLINK",
            NfsArguments::Read(_) => "READ",
            NfsArguments::Write(_) => "WRITE",
            NfsArguments::Create(_) => "CREATE",
            NfsArguments::MkDir(_) => "MKDIR",
            NfsArguments::SymLink(_) => "SYMLINK",
            NfsArguments::MkNod(_) => "MKNOD",
            NfsArguments::Remove(_) => "REMOVE",
            NfsArguments::RmDir(_) => "RMDIR",
            NfsArguments::Rename(_) => "RENAME",
            NfsArguments::Link(_) => "LINK",
            NfsArguments::ReadDir(_) => "READDIR",
            NfsArguments::ReadDirPlus(_) => "READDIRPLUS",
            NfsArguments::FsStat(_) => "FSSTAT",
            NfsArguments::FsInfo(_) => "FSINFO",
            NfsArguments::PathConf(_) => "PATHCONF",
            NfsArguments::Commit(_) => "COMMIT",
        }
    }

    fn error_from_response(response: &NfsRes) -> Option<vfs::Error> {
        match response {
            NfsRes::Null => None,
            NfsRes::GetAttr(Err(err)) => Some(err.error),
            NfsRes::SetAttr(Err(err)) => Some(err.error),
            NfsRes::LookUp(Err(err)) => Some(err.error),
            NfsRes::Access(Err(err)) => Some(err.error),
            NfsRes::ReadLink(Err(err)) => Some(err.error),
            NfsRes::Read(Err(err)) => Some(err.error),
            NfsRes::Write(Err(err)) => Some(err.error),
            NfsRes::Create(Err(err)) => Some(err.error),
            NfsRes::MkDir(Err(err)) => Some(err.error),
            NfsRes::SymLink(Err(err)) => Some(err.error),
            NfsRes::MkNod(Err(err)) => Some(err.error),
            NfsRes::Remove(Err(err)) => Some(err.error),
            NfsRes::RmDir(Err(err)) => Some(err.error),
            NfsRes::Rename(Err(err)) => Some(err.error),
            NfsRes::Link(Err(err)) => Some(err.error),
            NfsRes::ReadDir(Err(err)) => Some(err.error),
            NfsRes::ReadDirPlus(Err(err)) => Some(err.error),
            NfsRes::FsStat(Err(err)) => Some(err.error),
            NfsRes::FsInfo(Err(err)) => Some(err.error),
            NfsRes::PathConf(Err(err)) => Some(err.error),
            NfsRes::Commit(Err(err)) => Some(err.error),
            _ => None,
        }
    }
}
