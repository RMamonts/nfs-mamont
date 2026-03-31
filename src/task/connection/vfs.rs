use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tracing::error;

use crate::allocator::{Allocator, Impl, Slice};
use crate::context::ServerContext;
use crate::handles::HandleMap;
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

    async fn run(mut self) {
        while let Some(command) = self.command_receiver.recv().await {
            let NfsArgWrapper { header, proc } = command;
            let proc_name = Self::proc_name(&proc);

            let response = self.process_argument(proc).await;
            if let Some(error) = Self::error_from_response(&response) {
                error!(xid=header.xid, proc=%proc_name, error=?error, "nfs op failed");
            }

            let reply = ProcReply {
                xid: header.xid,
                proc_result: Ok(ProcResult::Nfs3(Box::new(response))),
            };

            if self.result_sender.send(reply).is_err() {
                return;
            }
        }
    }

    async fn process_argument(&self, proc: Box<NfsArguments>) -> NfsRes {
        match *proc {
            NfsArguments::Null => NfsRes::Null,

            NfsArguments::GetAttr(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => NfsRes::GetAttr(Err(vfs::get_attr::Fail { error })),
                Ok(lock) => {
                    let lock = lock.read().await;
                    NfsRes::GetAttr(self.backend.get_attr(lock.as_path()).await)
                }
            },

            NfsArguments::LookUp(args) => match self.handles.path_for_handle(&args.parent).await {
                Err(error) => NfsRes::LookUp(Err(vfs::lookup::Fail { error, dir_attr: None })),
                Ok(lock) => {
                    let path_parent = lock.write().await;

                    let mut path = path_parent.clone();
                    path.push(args.name.as_str());

                    match self.backend.lookup(path.as_path()).await {
                        Err(err) => NfsRes::LookUp(Err(err)),
                        Ok(ok) => {
                            let handle = match self.handles.create_handle(path.as_path()).await {
                                Ok(handle) => handle,
                                Err(_) => {
                                    unreachable!("handle creation failed, fs consistency is broken")
                                }
                            };
                            NfsRes::LookUp(Ok(vfs::lookup::Success {
                                file: handle,
                                file_attr: ok.file_attr,
                                dir_attr: ok.dir_attr,
                            }))
                        }
                    }
                }
            },

            NfsArguments::Create(args) => {
                match self.handles.path_for_handle(&args.object.dir).await {
                    Err(error) => NfsRes::Create(Err(vfs::create::Fail {
                        error,
                        wcc_data: WccData::default(),
                    })),
                    Ok(lock) => {
                        let path_parent = lock.write().await;

                        let mut path = path_parent.clone();
                        path.push(args.object.name.as_str());

                        match self.backend.create(path.as_path(), args.how).await {
                            Err(err) => NfsRes::Create(Err(err)),
                            Ok(ok) => {
                                //TODO(can we use option, if possible, or unreachable everywhere?)
                                let handle = self.handles.create_handle(path.as_path()).await.ok();
                                NfsRes::Create(Ok(vfs::create::Success {
                                    file: handle,
                                    attr: ok.attr,
                                    wcc_data: ok.wcc_data,
                                }))
                            }
                        }
                    }
                }
            }

            NfsArguments::MkDir(args) => {
                match self.handles.path_for_handle(&args.object.dir).await {
                    Err(error) => {
                        NfsRes::MkDir(Err(vfs::mk_dir::Fail { error, dir_wcc: WccData::default() }))
                    }
                    Ok(lock) => {
                        let path_parent = lock.write().await;

                        let mut path = path_parent.clone();
                        path.push(args.object.name.as_str());

                        match self.backend.mk_dir(path.as_path(), args.attr).await {
                            Err(err) => NfsRes::MkDir(Err(err)),
                            Ok(ok) => {
                                let handle = self.handles.create_handle(path.as_path()).await.ok();
                                NfsRes::MkDir(Ok(vfs::mk_dir::Success {
                                    file: handle,
                                    attr: ok.attr,
                                    wcc_data: ok.wcc_data,
                                }))
                            }
                        }
                    }
                }
            }

            NfsArguments::Remove(args) => {
                let handle = &args.object.dir.clone();

                match self.handles.path_for_handle(handle).await {
                    Err(error) => NfsRes::Remove(Err(vfs::remove::Fail {
                        error,
                        dir_wcc: WccData::default(),
                    })),
                    Ok(lock) => {
                        let path_parent = lock.write().await;

                        let mut path = path_parent.clone();
                        path.push(args.object.name.as_str());

                        match self.backend.remove(path.as_path()).await {
                            Err(err) => NfsRes::Remove(Err(err)),
                            Ok(ok) => {
                                // not sure what to do, should be unreachable
                                match self.handles.remove_handle(handle).await {
                                    Ok(_) => NfsRes::Remove(Ok(ok)),
                                    Err(_) => unreachable!(
                                        "handle remove failed, fs consistency is broken"
                                    ),
                                }
                            }
                        }
                    }
                }
            }

            NfsArguments::RmDir(args) => {
                let handle = &args.object.dir.clone();
                match self.handles.path_for_handle(handle).await {
                    Err(error) => {
                        NfsRes::RmDir(Err(vfs::rm_dir::Fail { error, dir_wcc: WccData::default() }))
                    }
                    Ok(lock) => {
                        let path_parent = lock.write().await;

                        let mut path = path_parent.clone();
                        path.push(args.object.name.as_str());

                        match self.backend.rm_dir(path.as_path()).await {
                            Err(err) => NfsRes::RmDir(Err(err)),
                            Ok(ok) => {
                                // not sure what to do, should be unreachable
                                match self.handles.remove_handle(handle).await {
                                    Ok(_) => NfsRes::RmDir(Ok(ok)),
                                    Err(_) => unreachable!(
                                        "handle remove failed, fs consistency is broken"
                                    ),
                                }
                            }
                        }
                    }
                }
            }

            NfsArguments::Rename(args) => {
                let from_dir = match self.handles.path_for_handle(&args.from.dir).await {
                    Ok(dir) => dir,
                    Err(error) => {
                        return NfsRes::Rename(Err(vfs::rename::Fail {
                            error,
                            from_dir_wcc: WccData::default(),
                            to_dir_wcc: WccData::default(),
                        }))
                    }
                };

                let to_dir = match self.handles.path_for_handle(&args.to.dir).await {
                    Ok(dir) => dir,
                    Err(error) => {
                        return NfsRes::Rename(Err(vfs::rename::Fail {
                            error,
                            from_dir_wcc: WccData::default(),
                            to_dir_wcc: WccData::default(),
                        }))
                    }
                };
                let lock_from = from_dir.write().await;
                let lock_to = to_dir.write().await;

                let mut from = lock_from.clone();
                from.push(args.from.name.as_str());

                let mut to = lock_to.clone();
                to.push(args.to.name.as_str());

                match self.backend.rename(from.as_path(), to.as_path()).await {
                    Err(err) => NfsRes::Rename(Err(err)),
                    Ok(ok) => match self.handles.rename_path(from.as_path(), to.as_path()).await {
                        Ok(_) => NfsRes::Rename(Ok(ok)),
                        Err(_) => unreachable!("handle rename failed, fs consistency is broken"),
                    },
                }
            }

            NfsArguments::Link(args) => {
                let object = match self.handles.path_for_handle(&args.file).await {
                    Ok(dir) => dir,
                    Err(error) => {
                        return NfsRes::Link(Err(vfs::link::Fail {
                            error,
                            file_attr: None,
                            dir_wcc: WccData::default(),
                        }))
                    }
                };

                let parent = match self.handles.path_for_handle(&args.link.dir).await {
                    Ok(dir) => dir,
                    Err(error) => {
                        return NfsRes::Link(Err(vfs::link::Fail {
                            error,
                            file_attr: None,
                            dir_wcc: WccData::default(),
                        }))
                    }
                };

                let real = object.read().await;

                let mut path = parent.write().await.clone();
                path.push(args.link.name.as_str());

                match self.backend.link(path.as_path(), real.as_path()).await {
                    Err(err) => NfsRes::Link(Err(err)),
                    Ok(ok) => {
                        let _handle = self.handles.create_handle(path.as_path()).await.ok();
                        NfsRes::Link(Ok(vfs::link::Success {
                            file_attr: ok.file_attr,
                            dir_wcc: ok.dir_wcc,
                        }))
                    }
                }
            }

            NfsArguments::SymLink(args) => {
                //TODO("check that path really exist")

                let parent = match self.handles.path_for_handle(&args.object.dir).await {
                    Ok(dir) => dir,
                    Err(error) => {
                        return NfsRes::Link(Err(vfs::link::Fail {
                            error,
                            file_attr: None,
                            dir_wcc: WccData::default(),
                        }))
                    }
                };

                let mut path = parent.write().await.clone();
                path.push(args.object.name.as_str());

                let obj = args.path.clone();
                match self.backend.symlink(path.as_path(), obj.as_path(), args.attr).await {
                    Err(err) => NfsRes::SymLink(Err(err)),
                    Ok(ok) => {
                        let handle = self.handles.create_handle(path.as_path()).await.ok();
                        NfsRes::SymLink(Ok(vfs::symlink::Success {
                            file: handle,
                            attr: ok.attr,
                            wcc_data: ok.wcc_data,
                        }))
                    }
                }
            }
            NfsArguments::SetAttr(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => NfsRes::SetAttr(Err(vfs::set_attr::Fail {
                    error,
                    wcc_data: WccData::default(),
                })),
                Ok(lock) => {
                    let path = lock.read().await;
                    NfsRes::SetAttr(
                        self.backend.set_attr(path.as_path(), args.new_attr, args.guard).await,
                    )
                }
            },

            NfsArguments::Access(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => NfsRes::Access(Err(vfs::access::Fail { error, object_attr: None })),
                Ok(lock) => {
                    let path = lock.read().await;
                    NfsRes::Access(self.backend.access(path.as_path(), args.mask).await)
                }
            },

            NfsArguments::ReadLink(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => {
                    NfsRes::ReadLink(Err(vfs::read_link::Fail { symlink_attr: None, error }))
                }
                Ok(lock) => {
                    let path = lock.read().await;
                    NfsRes::ReadLink(self.backend.read_link(path.as_path()).await)
                }
            },

            NfsArguments::Read(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => NfsRes::Read(Err(vfs::read::Fail { error, file_attr: None })),
                Ok(lock) => {
                    let data_result = if args.count == 0 {
                        Ok(Slice::empty())
                    } else {
                        let requested_size = NonZeroUsize::new(args.count as usize).unwrap();

                        let mut allocator = self.allocator.lock().await;
                        allocator
                            .allocate(requested_size)
                            .await
                            .ok_or(vfs::read::Fail { error: vfs::Error::TooSmall, file_attr: None })
                    };

                    match data_result {
                        Ok(data) => {
                            let path = lock.read().await;
                            NfsRes::Read(
                                self.backend
                                    .read(path.as_path(), args.offset, args.count, data)
                                    .await,
                            )
                        }
                        Err(err) => NfsRes::Read(Err(err)),
                    }
                }
            },

            NfsArguments::Write(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => {
                    NfsRes::Write(Err(vfs::write::Fail { error, wcc_data: WccData::default() }))
                }
                Ok(lock) => {
                    let path = lock.read().await;
                    NfsRes::Write(
                        self.backend
                            .write(path.as_path(), args.offset, args.size, args.stable, args.data)
                            .await,
                    )
                }
            },
            //remake to write and change
            NfsArguments::MkNod(args) => {
                let parent = match self.handles.path_for_handle(&args.object.dir).await {
                    Ok(dir) => dir,
                    Err(error) => {
                        return NfsRes::MkNod(Err(vfs::mk_node::Fail {
                            error,
                            dir_wcc: WccData::default(),
                        }))
                    }
                };

                let mut path = parent.write().await.clone();
                path.push(args.object.name.as_str());

                match self.backend.mk_node(path.as_path(), args.what).await {
                    Err(err) => NfsRes::MkNod(Err(err)),
                    Ok(ok) => {
                        let handle = self.handles.create_handle(path.as_path()).await.ok();
                        NfsRes::MkNod(Ok(vfs::mk_node::Success {
                            file: handle,
                            attr: ok.attr,
                            wcc_data: ok.wcc_data,
                        }))
                    }
                }
            }

            NfsArguments::ReadDir(args) => match self.handles.path_for_handle(&args.dir).await {
                Err(error) => NfsRes::ReadDir(Err(vfs::read_dir::Fail { error, dir_attr: None })),
                Ok(lock) => {
                    let path = lock.write().await;
                    NfsRes::ReadDir(
                        self.backend
                            .read_dir(path.as_path(), args.cookie, args.cookie_verifier, args.count)
                            .await,
                    )
                }
            },

            NfsArguments::ReadDirPlus(args) => {
                match self.handles.path_for_handle(&args.dir).await {
                    Err(error) => {
                        NfsRes::ReadDirPlus(Err(vfs::read_dir_plus::Fail { error, dir_attr: None }))
                    }
                    Ok(lock) => {
                        let path = lock.write().await;
                        NfsRes::ReadDirPlus(
                            self.backend
                                .read_dir_plus(
                                    path.as_path(),
                                    args.cookie,
                                    args.cookie_verifier,
                                    args.dir_count,
                                    args.max_count,
                                )
                                .await,
                        )
                    }
                }
            }

            NfsArguments::FsStat(args) => match self.handles.path_for_handle(&args.root).await {
                Err(error) => NfsRes::FsStat(Err(vfs::fs_stat::Fail { error, root_attr: None })),
                Ok(lock) => {
                    let path = lock.read().await;
                    NfsRes::FsStat(self.backend.fs_stat(path.as_path()).await)
                }
            },

            NfsArguments::FsInfo(args) => match self.handles.path_for_handle(&args.root).await {
                Err(error) => NfsRes::FsInfo(Err(vfs::fs_info::Fail { error, root_attr: None })),
                Ok(lock) => {
                    let path = lock.read().await;
                    NfsRes::FsInfo(self.backend.fs_info(path.as_path()).await)
                }
            },

            NfsArguments::PathConf(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => {
                    NfsRes::PathConf(Err(vfs::path_conf::Fail { error, file_attr: None }))
                }
                Ok(lock) => {
                    let path = lock.read().await;
                    NfsRes::PathConf(self.backend.path_conf(path.as_path()).await)
                }
            },

            NfsArguments::Commit(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => {
                    NfsRes::Commit(Err(vfs::commit::Fail { error, file_wcc: WccData::default() }))
                }
                Ok(lock) => {
                    let path = lock.read().await;
                    NfsRes::Commit(
                        self.backend.commit(path.as_path(), args.offset, args.count).await,
                    )
                }
            },
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
