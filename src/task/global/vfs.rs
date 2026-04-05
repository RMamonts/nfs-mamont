//! VfsTask is the executor for NFSv3 RPC operations.
//! It receives parsed NFS arguments, resolves handles to paths, applies the
//! required locking protocol, invokes the backend VFS, updates HandleMap, and
//! sends replies back to the connection writer.
//!
//! ## Concurrency and locking model
//!
//! VfsTask is responsible for **all synchronization** around HandleMap and
//! filesystem structure modifications.
//!
//! ## Non-recursive structural operations
//!
//! REMOVE and RENAME update only the specific path being modified.
//! Descendants are not rewritten; they remain valid and will be lazily updated
//! when accessed.
//!
use async_channel::{Receiver, Sender};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, warn};

use crate::allocator::{Allocator, Impl, Slice};
use crate::handles::{ensure_name_allowed, HandleMap};
use crate::parser::{NfsArgWrapper, NfsArguments};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::file::Handle;
use crate::vfs::{self, NfsRes, Vfs, WccData};

/// One queued NFS procedure: parsed arguments and a channel to send the result.
pub type VfsCommand = (NfsArgWrapper, UnboundedSender<ProcReply>);
/// Sender to enqueue work in the pool.
pub type VfsCommandSender = Sender<VfsCommand>;
/// Receiver from the pool, each worker competes for the same command stream.
type VfsCommandReceiver = Receiver<VfsCommand>;

/// Fixed-size pool of [`VfsTask`] workers fed from a single unbounded command channel.
pub struct VfsPool {
    /// Sender to enqueue work in the pool for execution.
    sender: VfsCommandSender,
}

impl VfsPool {
    /// Creates a new [`VfsPool`] with the given number of workers.
    ///
    /// # Parameters
    ///
    /// - `num` --- number of workers to create
    /// - `backend` --- shared filesystem implementation
    /// - `allocator` --- allocator used for read buffers
    ///
    /// # Returns
    ///
    /// A new [`VfsPool`] with the given number of workers.
    pub fn new(
        num: NonZeroUsize,
        backend: Arc<dyn Vfs + Send + Sync + 'static>,
        handles: Arc<HandleMap>,
        allocator: Arc<Impl>,
    ) -> Self {
        let (tx, rx) = async_channel::unbounded::<VfsCommand>();

        (0..num.get()).for_each(|_| {
            let rx_clone = rx.clone();
            VfsTask::new(Arc::clone(&backend), handles.clone(), Arc::clone(&allocator), rx_clone)
                .spawn();
        });

        Self { sender: tx }
    }

    /// Returns a clone of the command sender for enqueueing work in the pool.
    pub fn sender(&self) -> VfsCommandSender {
        self.sender.clone()
    }
}

impl Drop for VfsPool {
    /// Closes the pool's sender so workers stop after channel is empty.
    fn drop(&mut self) {
        self.sender.close();
    }
}

/// Task that executes NFS procedures against [`Vfs`] and sends the result to the writer pipeline.
pub struct VfsTask {
    /// Shared filesystem implementation.
    backend: Arc<dyn Vfs + Send + Sync + 'static>,
    handles: Arc<HandleMap>,
    /// Allocator used for read buffers.
    allocator: Arc<Impl>,
    /// Shared receiver from the pool, each worker competes for the same command stream.
    command_receiver: VfsCommandReceiver,
}

struct FailRes;

impl VfsTask {
    /// Builds a worker that reads commands from the pool and executes them.
    ///
    /// # Parameters
    ///
    /// - `backend` --- shared filesystem implementation
    /// - `allocator` --- allocator used for read buffers
    /// - `command_receiver` --- receiver from the pool
    ///
    /// # Returns
    ///
    /// A new [`VfsTask`] that reads commands from the pool and executes them.
    pub fn new(
        backend: Arc<dyn Vfs + Send + Sync + 'static>,
        handles: Arc<HandleMap>,
        allocator: Arc<Impl>,
        command_receiver: VfsCommandReceiver,
    ) -> Self {
        Self { backend, handles, allocator, command_receiver }
    }

    /// Spawns a [`VfsTask`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    /// Consumes commands until the channel closes, dispatching each NFS op and sending replies.
    async fn run(self) {
        while let Ok((command, tx)) = self.command_receiver.recv().await {
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

            // Write task may already be closed; then this connection pipeline is done.
            if tx.send(reply).is_err() {
                warn!("writer task closed, connection pipeline is done");
            }
        }
    }

    fn create_handle_or_panic(&self, path: &Path) -> Handle {
        match self.handles.create_handle(path) {
            Ok(handle) => handle,
            Err(err) => unreachable!("handle creation failed, fs consistency is broken: {:?}", err),
        }
    }
    fn create_handle_or_none(&self, path: &Path) -> Option<Handle> {
        // `post_op_fh3` from RFC allow to return Option
        // it is used in case vfs successfully finish operation, but handle could not be created
        self.handles.create_handle(path).ok()
    }

    fn remove_path(&self, path: &Path) {
        // since map could be concurrently updated, ignore errors
        let _ = self.handles.remove_path(path);
    }

    fn join_name(parent: &Path, name: &str) -> PathBuf {
        let mut path = parent.to_path_buf();
        path.push(name);
        path
    }

    async fn process_argument(&self, proc: Box<NfsArguments>) -> NfsRes {
        match *proc {
            NfsArguments::Null => NfsRes::Null,

            NfsArguments::GetAttr(args) => match self.handles.path_for_handle(&args.file) {
                Err(error) => FailRes::get_attr(error),
                Ok(path) => NfsRes::GetAttr(self.backend.get_attr(path.as_path()).await),
            },

            NfsArguments::LookUp(args) => match self.handles.path_for_handle(&args.parent) {
                Err(error) => FailRes::lookup(error),
                Ok(path_parent) => {
                    if let Err(error) = ensure_name_allowed(&args.name) {
                        return FailRes::lookup(error);
                    }

                    let path = Self::join_name(path_parent.as_path(), args.name.as_str());

                    match self.backend.lookup(path.as_path()).await {
                        Err(err) => NfsRes::LookUp(Err(err)),
                        Ok(ok) => NfsRes::LookUp(Ok(vfs::lookup::Success {
                            file: self.create_handle_or_panic(&path),
                            file_attr: ok.file_attr,
                            dir_attr: ok.dir_attr,
                        })),
                    }
                }
            },

            NfsArguments::Create(args) => match self.handles.path_for_handle(&args.object.dir) {
                Err(error) => FailRes::create(error),
                Ok(path_parent) => {
                    if let Err(error) = ensure_name_allowed(&args.object.name) {
                        return FailRes::create(error);
                    }
                    let path = Self::join_name(path_parent.as_path(), args.object.name.as_str());

                    match self.backend.create(path.as_path(), args.how).await {
                        Err(err) => NfsRes::Create(Err(err)),
                        Ok(ok) => NfsRes::Create(Ok(vfs::create::Success {
                            file: self.create_handle_or_none(&path),
                            attr: ok.attr,
                            wcc_data: ok.wcc_data,
                        })),
                    }
                }
            },

            NfsArguments::MkDir(args) => match self.handles.path_for_handle(&args.object.dir) {
                Err(error) => FailRes::mk_dir(error),
                Ok(path_parent) => {
                    if let Err(error) = ensure_name_allowed(&args.object.name) {
                        return FailRes::mk_dir(error);
                    }

                    let path = Self::join_name(path_parent.as_path(), args.object.name.as_str());

                    match self.backend.mk_dir(path.as_path(), args.attr).await {
                        Err(err) => NfsRes::MkDir(Err(err)),
                        Ok(ok) => NfsRes::MkDir(Ok(vfs::mk_dir::Success {
                            file: self.create_handle_or_none(&path),
                            attr: ok.attr,
                            wcc_data: ok.wcc_data,
                        })),
                    }
                }
            },

            NfsArguments::Remove(args) => match self.handles.path_for_handle(&args.object.dir) {
                Err(error) => FailRes::remove(error),
                Ok(path_parent) => {
                    if let Err(error) = ensure_name_allowed(&args.object.name) {
                        return FailRes::remove(error);
                    }
                    let path = Self::join_name(path_parent.as_path(), args.object.name.as_str());

                    // currently there is no way of notifying MountServer
                    if HandleMap::is_root(path.as_path()) {
                        return FailRes::remove(vfs::Error::Permission);
                    }
                    let handle = match self.handles.handle_for_path(path.as_path()) {
                        Ok(handle) => handle,
                        Err(error) => return FailRes::remove(error),
                    };

                    let path = match self.handles.path_for_handle(&handle) {
                        Err(error) => return FailRes::remove(error),
                        Ok(buf) => buf,
                    };

                    match self.backend.remove(path.as_path()).await {
                        Err(err) => NfsRes::Remove(Err(err)),
                        Ok(ok) => {
                            self.remove_path(path.as_path());
                            NfsRes::Remove(Ok(ok))
                        }
                    }
                }
            },

            NfsArguments::RmDir(args) => match self.handles.path_for_handle(&args.object.dir) {
                Err(error) => FailRes::rm_dir(error),
                Ok(path_parent) => {
                    if let Err(error) = ensure_name_allowed(&args.object.name) {
                        return FailRes::rm_dir(error);
                    }

                    let path = Self::join_name(path_parent.as_path(), args.object.name.as_str());
                    if HandleMap::is_root(path.as_path()) {
                        return FailRes::rm_dir(vfs::Error::Permission);
                    }
                    let handle = match self.handles.handle_for_path(path.as_path()) {
                        Ok(handle) => handle,
                        Err(vfs::Error::StaleFile) => return FailRes::rm_dir(vfs::Error::NoEntry),
                        Err(error) => return FailRes::rm_dir(error),
                    };
                    let path = match self.handles.path_for_handle(&handle) {
                        Err(error) => return FailRes::rm_dir(error),
                        Ok(path) => path,
                    };

                    match self.backend.rm_dir(path.as_path()).await {
                        Err(err) => NfsRes::RmDir(Err(err)),
                        Ok(ok) => {
                            self.remove_path(path.as_path());
                            NfsRes::RmDir(Ok(ok))
                        }
                    }
                }
            },

            NfsArguments::Rename(args) => {
                let from_dir = match self.handles.path_for_handle(&args.from.dir) {
                    Ok(dir) => dir,
                    Err(error) => return FailRes::rename(error),
                };

                let to_dir = match self.handles.path_for_handle(&args.to.dir) {
                    Ok(dir) => dir,
                    Err(error) => return FailRes::rename(error),
                };

                if let Err(error) = ensure_name_allowed(&args.to.name) {
                    return FailRes::rename(error);
                }
                if let Err(error) = ensure_name_allowed(&args.from.name) {
                    return FailRes::rename(error);
                }

                let from = Self::join_name(from_dir.as_path(), args.from.name.as_str());
                let to = Self::join_name(to_dir.as_path(), args.to.name.as_str());

                if HandleMap::is_root(from.as_path()) || HandleMap::is_root(to.as_path()) {
                    return FailRes::rename(vfs::Error::Permission);
                }

                let from_handle = match self.handles.handle_for_path(from.as_path()) {
                    Ok(handle) => handle,
                    Err(error) => return FailRes::rename(error),
                };
                let to_handle = match self.handles.handle_for_path(to.as_path()) {
                    Ok(handle) => Some(handle),
                    Err(vfs::Error::StaleFile) => None,
                    Err(error) => return FailRes::rename(error),
                };

                match self.backend.rename(from.as_path(), to.as_path()).await {
                    Err(err) => NfsRes::Rename(Err(err)),
                    Ok(ok) => match self.handles.rename_path(
                        from.as_path(),
                        to.as_path(),
                        from_handle,
                        to_handle,
                    ) {
                        Ok(_) => NfsRes::Rename(Ok(ok)),
                        Err(error) => FailRes::rename(error),
                    },
                }
            }

            NfsArguments::Link(args) => {
                let object = match self.handles.path_for_handle(&args.file) {
                    Ok(dir) => dir,
                    Err(error) => return FailRes::link(error),
                };

                let parent_path = match self.handles.path_for_handle(&args.link.dir) {
                    Ok(dir) => dir,
                    Err(error) => return FailRes::link(error),
                };

                if let Err(error) = ensure_name_allowed(&args.link.name) {
                    return FailRes::link(error);
                }

                let path = Self::join_name(parent_path.as_path(), args.link.name.as_str());

                match self.backend.link(path.as_path(), object.as_path()).await {
                    Err(err) => NfsRes::Link(Err(err)),
                    Ok(ok) => {
                        let _handle = self.create_handle_or_none(&path);
                        NfsRes::Link(Ok(vfs::link::Success {
                            file_attr: ok.file_attr,
                            dir_wcc: ok.dir_wcc,
                        }))
                    }
                }
            }

            NfsArguments::SymLink(args) => {
                let mut path = match self.handles.path_for_handle(&args.object.dir) {
                    Ok(dir) => dir,
                    Err(error) => return FailRes::symlink(error),
                };

                if let Err(error) = ensure_name_allowed(&args.object.name) {
                    return FailRes::symlink(error);
                }

                path.push(args.object.name.as_str());

                let obj = args.path.clone();

                match self.backend.symlink(path.as_path(), obj.as_path(), args.attr).await {
                    Err(err) => NfsRes::SymLink(Err(err)),
                    Ok(ok) => NfsRes::SymLink(Ok(vfs::symlink::Success {
                        file: self.create_handle_or_none(&path),
                        attr: ok.attr,
                        wcc_data: ok.wcc_data,
                    })),
                }
            }
            NfsArguments::SetAttr(args) => match self.handles.path_for_handle(&args.file) {
                Err(error) => FailRes::set_attr(error),
                Ok(path) => NfsRes::SetAttr(
                    self.backend.set_attr(path.as_path(), args.new_attr, args.guard).await,
                ),
            },

            NfsArguments::Access(args) => match self.handles.path_for_handle(&args.file) {
                Err(error) => FailRes::access(error),
                Ok(path) => NfsRes::Access(self.backend.access(path.as_path(), args.mask).await),
            },

            NfsArguments::ReadLink(args) => match self.handles.path_for_handle(&args.file) {
                Err(error) => FailRes::read_link(error),
                Ok(path) => NfsRes::ReadLink(self.backend.read_link(path.as_path()).await),
            },

            NfsArguments::Read(args) => match self.handles.path_for_handle(&args.file) {
                Err(error) => FailRes::read(error),
                Ok(path) => {
                    let data_result = if args.count == 0 {
                        Ok(Slice::empty())
                    } else {
                        let requested_size = NonZeroUsize::new(args.count as usize).unwrap();

                        self.allocator
                            .allocate(requested_size)
                            .await
                            .ok_or(vfs::read::Fail { error: vfs::Error::TooSmall, file_attr: None })
                    };

                    match data_result {
                        Ok(data) => NfsRes::Read(
                            self.backend.read(path.as_path(), args.offset, args.count, data).await,
                        ),
                        Err(err) => NfsRes::Read(Err(err)),
                    }
                }
            },

            NfsArguments::Write(args) => match self.handles.path_for_handle(&args.file) {
                Err(error) => FailRes::write(error),
                Ok(path) => NfsRes::Write(
                    self.backend
                        .write(path.as_path(), args.offset, args.size, args.stable, args.data)
                        .await,
                ),
            },
            NfsArguments::MkNod(args) => {
                let mut path = match self.handles.path_for_handle(&args.object.dir) {
                    Ok(dir) => dir,
                    Err(error) => return FailRes::mk_nod(error),
                };

                if let Err(error) = ensure_name_allowed(&args.object.name) {
                    return FailRes::mk_nod(error);
                }

                path.push(args.object.name.as_str());

                match self.backend.mk_node(path.as_path(), args.what).await {
                    Err(err) => NfsRes::MkNod(Err(err)),
                    Ok(ok) => NfsRes::MkNod(Ok(vfs::mk_node::Success {
                        file: self.create_handle_or_none(&path),
                        attr: ok.attr,
                        wcc_data: ok.wcc_data,
                    })),
                }
            }

            NfsArguments::ReadDir(args) => match self.handles.path_for_handle(&args.dir) {
                Err(error) => FailRes::read_dir(error),
                Ok(path) => NfsRes::ReadDir(
                    match self
                        .backend
                        .read_dir(path.as_path(), args.cookie, args.cookie_verifier, args.count)
                        .await
                    {
                        Ok(mut ok) => {
                            for entry in ok.entries.iter_mut() {
                                let name = &entry.file_name;
                                let mut entry_path = path.clone();
                                entry_path.push(name.as_str());
                                match self.handles.create_handle(&entry_path) {
                                    Ok(_) => continue,
                                    Err(error) => return FailRes::read_dir(error),
                                }
                            }
                            Ok(ok)
                        }
                        error => error,
                    },
                ),
            },

            NfsArguments::ReadDirPlus(args) => match self.handles.path_for_handle(&args.dir) {
                Err(error) => FailRes::read_dir_plus(error),
                Ok(path) => NfsRes::ReadDirPlus(
                    match self
                        .backend
                        .read_dir_plus(
                            path.as_path(),
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
                                match self.handles.create_handle(&entry_path) {
                                    Ok(handle) => entry.file_handle = Some(handle),
                                    Err(error) => return FailRes::read_dir_plus(error),
                                }
                            }
                            Ok(ok)
                        }
                        Err(error) => Err(error),
                    },
                ),
            },

            NfsArguments::FsStat(args) => match self.handles.path_for_handle(&args.root) {
                Err(error) => FailRes::fs_stat(error),
                Ok(path) => {
                    //TODO("root in args required to determine, which of mounted fs to use;
                    // so redirection to correct vfs should be implemented")

                    NfsRes::FsStat(self.backend.fs_stat(path.as_path()).await)
                }
            },

            NfsArguments::FsInfo(args) => match self.handles.path_for_handle(&args.root) {
                Err(error) => FailRes::fs_info(error),
                Ok(path) => NfsRes::FsInfo(self.backend.fs_info(path.as_path()).await),
            },

            NfsArguments::PathConf(args) => match self.handles.path_for_handle(&args.file) {
                Err(error) => FailRes::path_conf(error),
                Ok(path) => NfsRes::PathConf(self.backend.path_conf(path.as_path()).await),
            },

            NfsArguments::Commit(args) => match self.handles.path_for_handle(&args.file) {
                Err(error) => FailRes::commit(error),
                Ok(path) => NfsRes::Commit(
                    self.backend.commit(path.as_path(), args.offset, args.count).await,
                ),
            },
        }
    }

    /// Static label for logging/tracing for the given procedure variant.
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

    /// Returns the domain error when the NFS result variant is `Err`, if present.
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
