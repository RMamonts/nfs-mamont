//! VfsTask is the executor for NFSv3 RPC operations.
//! It receives parsed NFS arguments, resolves handles to paths, applies the
//! required locking protocol, invokes the backend VFS, updates HandleMap, and
//! sends replies back to the connection writer.
//!
//! ## Concurrency and locking model
//!
//! VfsTask is responsible for **all synchronization** around HandleMap and
//! filesystem structure modifications.
//! HandleMap itself is not atomic, so VfsTask ensures correctness by taking
//! appropriate locks before performing any operation.
//!
//! ### Write-locks
//!
//! A write-lock is taken whenever an operation may modify the filesystem
//! structure or the HandleMap state:
//!
//! - CREATE
//! - MKDIR
//! - REMOVE
//! - RMDIR
//! - RENAME
//! - LINK
//! - SYMLINK
//! - MKNOD
//! - READDIR / READDIRPLUS
//!   (these create new handles for directory entries, so HandleMap is mutated)
//!
//! These locks ensure that multi-table updates inside HandleMap behave as a
//! logically atomic unit.
//!
//! ### Read-locks
//!
//! Read-only operations take only a read-lock on the path:
//!
//! - GETATTR
//! - ACCESS
//! - READLINK
//! - READ
//! - WRITE (does not modify directory structure)
//! - FSSTAT
//! - FSINFO
//! - PATHCONF
//! - COMMIT
//!
//! These operations do not change HandleMap or directory structure.
//!
//! ## Non-recursive structural operations
//!
//! REMOVE and RENAME update only the specific path being modified.
//! Descendants are not rewritten; they remain valid and will be lazily updated
//! when accessed.
//!
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tracing::error;

use crate::allocator::{Allocator, Impl, Slice};
use crate::context::ServerContext;
use crate::handles::{ensure_name_allowed, HandleMap};
use crate::parser::{NfsArgWrapper, NfsArguments};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::file::Handle;
use crate::vfs::{self, NfsRes, Vfs, WccData};

/// Process RPC commands, sends operation results to [`crate::task::connection::write::WriteTask`].
pub struct VfsTask {
    backend: Arc<dyn Vfs + Send + Sync + 'static>,
    allocator: Arc<Mutex<Impl>>,
    handles: Arc<HandleMap>,
    command_receiver: UnboundedReceiver<NfsArgWrapper>,
    result_sender: UnboundedSender<ProcReply>,
}

struct FailRes;

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

    async fn create_handle_or_panic(&self, path: &Path) -> Handle {
        match self.handles.create_handle(path).await {
            Ok(handle) => handle,
            Err(err) => unreachable!("handle creation failed, fs consistency is broken: {:?}", err),
        }
    }

    async fn remove_path_or_panic(&self, path: &Path) {
        if let Err(err) = self.handles.remove_path(path).await {
            unreachable!("handle remove failed, fs consistency is broken: {:?}", err);
        }
    }

    fn join_name(parent: &Path, name: &str) -> PathBuf {
        let mut path = parent.to_path_buf();
        path.push(name);
        path
    }
    fn is_root(path: &Path) -> bool {
        path.as_os_str().is_empty()
    }

    fn to_full_path(&self, relative: &Path) -> PathBuf {
        self.handles.to_full_path(relative)
    }

    async fn process_argument(&self, proc: Box<NfsArguments>) -> NfsRes {
        match *proc {
            NfsArguments::Null => NfsRes::Null,

            NfsArguments::GetAttr(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => FailRes::get_attr(error),
                Ok(lock) => {
                    let path = lock.read().await;
                    let full_path = self.to_full_path(path.as_path());
                    NfsRes::GetAttr(self.backend.get_attr(full_path.as_path()).await)
                }
            },

            NfsArguments::LookUp(args) => match self.handles.path_for_handle(&args.parent).await {
                Err(error) => FailRes::lookup(error),
                Ok(lock) => {
                    if let Err(error) = ensure_name_allowed(&args.name) {
                        return FailRes::lookup(error);
                    }

                    let path_parent = lock.write().await;
                    let path = Self::join_name(path_parent.as_path(), args.name.as_str());
                    let full_path = self.to_full_path(path.as_path());

                    match self.backend.lookup(full_path.as_path()).await {
                        Err(err) => NfsRes::LookUp(Err(err)),
                        Ok(ok) => NfsRes::LookUp(Ok(vfs::lookup::Success {
                            file: self.create_handle_or_panic(&path).await,
                            file_attr: ok.file_attr,
                            dir_attr: ok.dir_attr,
                        })),
                    }
                }
            },

            NfsArguments::Create(args) => {
                match self.handles.path_for_handle(&args.object.dir).await {
                    Err(error) => FailRes::create(error),
                    Ok(lock) => {
                        if let Err(error) = ensure_name_allowed(&args.object.name) {
                            return FailRes::create(error);
                        }

                        let path_parent = lock.write().await;
                        let path =
                            Self::join_name(path_parent.as_path(), args.object.name.as_str());
                        let full_path = self.to_full_path(path.as_path());

                        match self.backend.create(full_path.as_path(), args.how).await {
                            Err(err) => NfsRes::Create(Err(err)),
                            Ok(ok) => NfsRes::Create(Ok(vfs::create::Success {
                                file: Some(self.create_handle_or_panic(&path).await),
                                attr: ok.attr,
                                wcc_data: ok.wcc_data,
                            })),
                        }
                    }
                }
            }

            NfsArguments::MkDir(args) => match self.handles.path_for_handle(&args.object.dir).await
            {
                Err(error) => FailRes::mk_dir(error),
                Ok(lock) => {
                    if let Err(error) = ensure_name_allowed(&args.object.name) {
                        return FailRes::mk_dir(error);
                    }

                    let path_parent = lock.write().await;
                    let path = Self::join_name(path_parent.as_path(), args.object.name.as_str());
                    let full_path = self.to_full_path(path.as_path());

                    match self.backend.mk_dir(full_path.as_path(), args.attr).await {
                        Err(err) => NfsRes::MkDir(Err(err)),
                        Ok(ok) => NfsRes::MkDir(Ok(vfs::mk_dir::Success {
                            file: Some(self.create_handle_or_panic(&path).await),
                            attr: ok.attr,
                            wcc_data: ok.wcc_data,
                        })),
                    }
                }
            },

            NfsArguments::Remove(args) => {
                match self.handles.path_for_handle(&args.object.dir).await {
                    Err(error) => FailRes::remove(error),
                    Ok(lock) => {
                        if let Err(error) = ensure_name_allowed(&args.object.name) {
                            return FailRes::remove(error);
                        }

                        let path_parent = lock.write().await;
                        let path =
                            Self::join_name(path_parent.as_path(), args.object.name.as_str());

                        if Self::is_root(path.as_path()) {
                            return FailRes::remove(vfs::Error::Permission);
                        }
                        let handle = match self.handles.handle_for_path(path.as_path()).await {
                            Ok(handle) => handle,
                            Err(error) => return FailRes::remove(error),
                        };

                        let lock = match self.handles.path_for_handle(&handle).await {
                            Err(error) => return FailRes::remove(error),
                            Ok(lock) => lock,
                        };

                        let _ = lock.write().await;

                        let full_path = self.to_full_path(path.as_path());
                        match self.backend.remove(full_path.as_path()).await {
                            Err(err) => NfsRes::Remove(Err(err)),
                            Ok(ok) => {
                                self.remove_path_or_panic(path.as_path()).await;
                                NfsRes::Remove(Ok(ok))
                            }
                        }
                    }
                }
            }

            NfsArguments::RmDir(args) => {
                match self.handles.path_for_handle(&args.object.dir).await {
                    Err(error) => FailRes::rm_dir(error),
                    Ok(lock) => {
                        if let Err(error) = ensure_name_allowed(&args.object.name) {
                            return FailRes::rm_dir(error);
                        }

                        let path_parent = lock.write().await;
                        let path =
                            Self::join_name(path_parent.as_path(), args.object.name.as_str());
                        if Self::is_root(path.as_path()) {
                            return FailRes::rm_dir(vfs::Error::Permission);
                        }
                        let handle = match self.handles.handle_for_path(path.as_path()).await {
                            Ok(handle) => handle,
                            Err(vfs::Error::StaleFile) => {
                                return FailRes::rm_dir(vfs::Error::NoEntry)
                            }
                            Err(error) => return FailRes::rm_dir(error),
                        };
                        let lock = match self.handles.path_for_handle(&handle).await {
                            Err(error) => return FailRes::rm_dir(error),
                            Ok(lock) => lock,
                        };

                        let _ = lock.write().await;

                        let full_path = self.to_full_path(path.as_path());

                        match self.backend.rm_dir(full_path.as_path()).await {
                            Err(err) => NfsRes::RmDir(Err(err)),
                            Ok(ok) => {
                                self.remove_path_or_panic(path.as_path()).await;
                                NfsRes::RmDir(Ok(ok))
                            }
                        }
                    }
                }
            }

            NfsArguments::Rename(args) => {
                let from_dir = match self.handles.path_for_handle(&args.from.dir).await {
                    Ok(dir) => dir,
                    Err(error) => return FailRes::rename(error),
                };

                let to_dir = match self.handles.path_for_handle(&args.to.dir).await {
                    Ok(dir) => dir,
                    Err(error) => return FailRes::rename(error),
                };

                if let Err(error) = ensure_name_allowed(&args.to.name) {
                    return FailRes::rename(error);
                }
                if let Err(error) = ensure_name_allowed(&args.from.name) {
                    return FailRes::rename(error);
                }

                let (from, to) = if args.from.dir == args.to.dir {
                    let dir_lock = from_dir.write().await;
                    let from = Self::join_name(dir_lock.as_path(), args.from.name.as_str());
                    let to = Self::join_name(dir_lock.as_path(), args.to.name.as_str());
                    (from, to)
                } else if args.to.dir >= args.from.dir {
                    let from_lock = from_dir.write().await;
                    let from = Self::join_name(from_lock.as_path(), args.from.name.as_str());

                    let to_lock = to_dir.write().await;
                    let to = Self::join_name(to_lock.as_path(), args.to.name.as_str());
                    (from, to)
                } else {
                    let to_lock = to_dir.write().await;
                    let to = Self::join_name(to_lock.as_path(), args.to.name.as_str());

                    let from_lock = from_dir.write().await;
                    let from = Self::join_name(from_lock.as_path(), args.from.name.as_str());

                    (from, to)
                };
                if Self::is_root(from.as_path()) || Self::is_root(to.as_path()) {
                    return FailRes::rename(vfs::Error::Permission);
                }

                let from_handle = match self.handles.handle_for_path(from.as_path()).await {
                    Ok(handle) => handle,
                    Err(error) => return FailRes::rename(error),
                };
                let to_handle = match self.handles.handle_for_path(to.as_path()).await {
                    Ok(handle) => Some(handle),
                    Err(vfs::Error::StaleFile) => None,
                    Err(error) => return FailRes::rename(error),
                };

                let from_lock = match self.handles.path_for_handle(&from_handle).await {
                    Ok(lock) => lock,
                    Err(error) => return FailRes::rename(error),
                };

                if let Some(to_handle) = to_handle.as_ref() {
                    let to_lock = match self.handles.path_for_handle(to_handle).await {
                        Ok(lock) => lock,
                        Err(error) => return FailRes::rename(error),
                    };
                    if from_handle == *to_handle {
                        let _guard = from_lock.write().await;
                    } else if to_handle >= &from_handle {
                        let _from_guard = from_lock.write().await;
                        let _to_guard = to_lock.write().await;
                    } else {
                        let _to_guard = to_lock.write().await;
                        let _from_guard = from_lock.write().await;
                    }
                } else {
                    let _ = from_lock.write().await;
                }

                let from_full = self.to_full_path(from.as_path());
                let to_full = self.to_full_path(to.as_path());

                match self.backend.rename(from_full.as_path(), to_full.as_path()).await {
                    Err(err) => NfsRes::Rename(Err(err)),
                    Ok(ok) => match self
                        .handles
                        .rename_path(from.as_path(), to.as_path(), from_handle, to_handle)
                        .await
                    {
                        Ok(_) => NfsRes::Rename(Ok(ok)),
                        Err(_) => unreachable!("handle rename failed, fs consistency is broken"),
                    },
                }
            }

            NfsArguments::Link(args) => {
                let object = match self.handles.path_for_handle(&args.file).await {
                    Ok(dir) => dir,
                    Err(error) => return FailRes::link(error),
                };

                let parent = match self.handles.path_for_handle(&args.link.dir).await {
                    Ok(dir) => dir,
                    Err(error) => return FailRes::link(error),
                };

                if let Err(error) = ensure_name_allowed(&args.link.name) {
                    return FailRes::link(error);
                }
                let real = object.read().await;
                let real_full = self.to_full_path(real.as_path());

                let parent_path = parent.write().await;
                let path = Self::join_name(parent_path.as_path(), args.link.name.as_str());
                let full_path = self.to_full_path(path.as_path());

                match self.backend.link(full_path.as_path(), real_full.as_path()).await {
                    Err(err) => NfsRes::Link(Err(err)),
                    Ok(ok) => {
                        let _handle = self.create_handle_or_panic(&path).await;
                        NfsRes::Link(Ok(vfs::link::Success {
                            file_attr: ok.file_attr,
                            dir_wcc: ok.dir_wcc,
                        }))
                    }
                }
            }

            NfsArguments::SymLink(args) => {
                let parent = match self.handles.path_for_handle(&args.object.dir).await {
                    Ok(dir) => dir,
                    Err(error) => return FailRes::symlink(error),
                };

                if let Err(error) = ensure_name_allowed(&args.object.name) {
                    return FailRes::symlink(error);
                }

                let mut path = parent.write().await.clone();
                path.push(args.object.name.as_str());
                let full_path = self.to_full_path(path.as_path());

                let obj = args.path.clone();

                match self.backend.symlink(full_path.as_path(), obj.as_path(), args.attr).await {
                    Err(err) => NfsRes::SymLink(Err(err)),
                    Ok(ok) => {
                        let handle = self.create_handle_or_panic(&path).await;
                        NfsRes::SymLink(Ok(vfs::symlink::Success {
                            file: Some(handle),
                            attr: ok.attr,
                            wcc_data: ok.wcc_data,
                        }))
                    }
                }
            }
            NfsArguments::SetAttr(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => FailRes::set_attr(error),
                Ok(lock) => {
                    let path = lock.write().await;
                    let full_path = self.to_full_path(path.as_path());
                    NfsRes::SetAttr(
                        self.backend.set_attr(full_path.as_path(), args.new_attr, args.guard).await,
                    )
                }
            },

            NfsArguments::Access(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => FailRes::access(error),
                Ok(lock) => {
                    let path = lock.read().await;
                    let full_path = self.to_full_path(path.as_path());
                    NfsRes::Access(self.backend.access(full_path.as_path(), args.mask).await)
                }
            },

            NfsArguments::ReadLink(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => FailRes::read_link(error),
                Ok(lock) => {
                    let path = lock.read().await;
                    let full_path = self.to_full_path(path.as_path());
                    NfsRes::ReadLink(self.backend.read_link(full_path.as_path()).await)
                }
            },

            NfsArguments::Read(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => FailRes::read(error),
                Ok(lock) => {
                    let path = lock.read().await;
                    let full_path = self.to_full_path(path.as_path());

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
                        Ok(data) => NfsRes::Read(
                            self.backend
                                .read(full_path.as_path(), args.offset, args.count, data)
                                .await,
                        ),
                        Err(err) => NfsRes::Read(Err(err)),
                    }
                }
            },

            NfsArguments::Write(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => FailRes::write(error),
                Ok(lock) => {
                    // though it looks unlogic, we can allow parallel writes to file
                    let path = lock.read().await;
                    let full_path = self.to_full_path(path.as_path());
                    NfsRes::Write(
                        self.backend
                            .write(
                                full_path.as_path(),
                                args.offset,
                                args.size,
                                args.stable,
                                args.data,
                            )
                            .await,
                    )
                }
            },
            NfsArguments::MkNod(args) => {
                let parent = match self.handles.path_for_handle(&args.object.dir).await {
                    Ok(dir) => dir,
                    Err(error) => return FailRes::mk_nod(error),
                };

                if let Err(error) = ensure_name_allowed(&args.object.name) {
                    return FailRes::mk_nod(error);
                }

                let mut path = parent.write().await.clone();
                path.push(args.object.name.as_str());
                let full_path = self.to_full_path(path.as_path());

                match self.backend.mk_node(full_path.as_path(), args.what).await {
                    Err(err) => NfsRes::MkNod(Err(err)),
                    Ok(ok) => {
                        let handle = self.create_handle_or_panic(&path).await;
                        NfsRes::MkNod(Ok(vfs::mk_node::Success {
                            file: Some(handle),
                            attr: ok.attr,
                            wcc_data: ok.wcc_data,
                        }))
                    }
                }
            }

            NfsArguments::ReadDir(args) => match self.handles.path_for_handle(&args.dir).await {
                Err(error) => FailRes::read_dir(error),
                Ok(lock) => {
                    let path = lock.write().await;
                    let full_path = self.to_full_path(path.as_path());
                    NfsRes::ReadDir(
                        match self
                            .backend
                            .read_dir(
                                full_path.as_path(),
                                args.cookie,
                                args.cookie_verifier,
                                args.count,
                            )
                            .await
                        {
                            Ok(mut ok) => {
                                for entry in ok.entries.iter_mut() {
                                    let name = &entry.file_name;
                                    let mut entry_path = path.clone();
                                    entry_path.push(name.as_str());
                                    match self.handles.create_handle(&entry_path).await {
                                        Ok(_) => continue,
                                        Err(error) => return FailRes::read_dir(error),
                                    }
                                }
                                Ok(ok)
                            }
                            error => error,
                        },
                    )
                }
            },

            NfsArguments::ReadDirPlus(args) => {
                match self.handles.path_for_handle(&args.dir).await {
                    Err(error) => FailRes::read_dir_plus(error),
                    Ok(lock) => {
                        let path = lock.write().await;
                        let full_path = self.to_full_path(path.as_path());
                        NfsRes::ReadDirPlus(
                            match self
                                .backend
                                .read_dir_plus(
                                    full_path.as_path(),
                                    args.cookie,
                                    args.cookie_verifier,
                                    args.dir_count,
                                    args.max_count,
                                )
                                .await
                            {
                                Ok(mut ok) => {
                                    for entry in ok.entries.iter_mut() {
                                        let name = &entry.file_name;
                                        let mut entry_path = path.clone();
                                        entry_path.push(name.as_str());
                                        match self.handles.create_handle(&entry_path).await {
                                            Ok(handle) => entry.file_handle = Some(handle),
                                            Err(error) => return FailRes::read_dir_plus(error),
                                        }
                                    }
                                    Ok(ok)
                                }
                                Err(error) => Err(error),
                            },
                        )
                    }
                }
            }

            NfsArguments::FsStat(args) => match self.handles.path_for_handle(&args.root).await {
                Err(error) => FailRes::fs_stat(error),
                Ok(lock) => {
                    //TODO("root in args required to determine, which of mounted fs to use;
                    // so redirection to correct vfs should be implemented")
                    let path = lock.read().await;
                    let full_path = self.to_full_path(path.as_path());
                    NfsRes::FsStat(self.backend.fs_stat(full_path.as_path()).await)
                }
            },

            NfsArguments::FsInfo(args) => match self.handles.path_for_handle(&args.root).await {
                Err(error) => FailRes::fs_info(error),
                Ok(lock) => {
                    let path = lock.read().await;
                    let full_path = self.to_full_path(path.as_path());
                    NfsRes::FsInfo(self.backend.fs_info(full_path.as_path()).await)
                }
            },

            NfsArguments::PathConf(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => FailRes::path_conf(error),
                Ok(lock) => {
                    let path = lock.read().await;
                    let full_path = self.to_full_path(path.as_path());
                    NfsRes::PathConf(self.backend.path_conf(full_path.as_path()).await)
                }
            },

            NfsArguments::Commit(args) => match self.handles.path_for_handle(&args.file).await {
                Err(error) => FailRes::commit(error),
                Ok(lock) => {
                    let path = lock.read().await;
                    let full_path = self.to_full_path(path.as_path());
                    NfsRes::Commit(
                        self.backend.commit(full_path.as_path(), args.offset, args.count).await,
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

impl FailRes {
    fn get_attr(error: vfs::Error) -> NfsRes {
        NfsRes::GetAttr(Err(vfs::get_attr::Fail { error }))
    }

    fn set_attr(error: vfs::Error) -> NfsRes {
        NfsRes::SetAttr(Err(vfs::set_attr::Fail { error, wcc_data: WccData::default() }))
    }

    fn lookup(error: vfs::Error) -> NfsRes {
        NfsRes::LookUp(Err(vfs::lookup::Fail { error, dir_attr: None }))
    }

    fn access(error: vfs::Error) -> NfsRes {
        NfsRes::Access(Err(vfs::access::Fail { error, object_attr: None }))
    }

    fn read_link(error: vfs::Error) -> NfsRes {
        NfsRes::ReadLink(Err(vfs::read_link::Fail { symlink_attr: None, error }))
    }

    fn read(error: vfs::Error) -> NfsRes {
        NfsRes::Read(Err(vfs::read::Fail { error, file_attr: None }))
    }

    fn write(error: vfs::Error) -> NfsRes {
        NfsRes::Write(Err(vfs::write::Fail { error, wcc_data: WccData::default() }))
    }

    fn create(error: vfs::Error) -> NfsRes {
        NfsRes::Create(Err(vfs::create::Fail { error, wcc_data: WccData::default() }))
    }

    fn mk_dir(error: vfs::Error) -> NfsRes {
        NfsRes::MkDir(Err(vfs::mk_dir::Fail { error, dir_wcc: WccData::default() }))
    }

    fn remove(error: vfs::Error) -> NfsRes {
        NfsRes::Remove(Err(vfs::remove::Fail { error, dir_wcc: WccData::default() }))
    }

    fn rm_dir(error: vfs::Error) -> NfsRes {
        NfsRes::RmDir(Err(vfs::rm_dir::Fail { error, dir_wcc: WccData::default() }))
    }

    fn rename(error: vfs::Error) -> NfsRes {
        NfsRes::Rename(Err(vfs::rename::Fail {
            error,
            from_dir_wcc: WccData::default(),
            to_dir_wcc: WccData::default(),
        }))
    }

    fn link(error: vfs::Error) -> NfsRes {
        NfsRes::Link(Err(vfs::link::Fail { error, file_attr: None, dir_wcc: WccData::default() }))
    }

    fn symlink(error: vfs::Error) -> NfsRes {
        NfsRes::SymLink(Err(vfs::symlink::Fail { error, dir_wcc: WccData::default() }))
    }

    fn mk_nod(error: vfs::Error) -> NfsRes {
        NfsRes::MkNod(Err(vfs::mk_node::Fail { error, dir_wcc: WccData::default() }))
    }

    fn read_dir(error: vfs::Error) -> NfsRes {
        NfsRes::ReadDir(Err(vfs::read_dir::Fail { error, dir_attr: None }))
    }

    fn read_dir_plus(error: vfs::Error) -> NfsRes {
        NfsRes::ReadDirPlus(Err(vfs::read_dir_plus::Fail { error, dir_attr: None }))
    }

    fn fs_stat(error: vfs::Error) -> NfsRes {
        NfsRes::FsStat(Err(vfs::fs_stat::Fail { error, root_attr: None }))
    }

    fn fs_info(error: vfs::Error) -> NfsRes {
        NfsRes::FsInfo(Err(vfs::fs_info::Fail { error, root_attr: None }))
    }

    fn path_conf(error: vfs::Error) -> NfsRes {
        NfsRes::PathConf(Err(vfs::path_conf::Fail { error, file_attr: None }))
    }

    fn commit(error: vfs::Error) -> NfsRes {
        NfsRes::Commit(Err(vfs::commit::Fail { error, file_wcc: WccData::default() }))
    }
}
