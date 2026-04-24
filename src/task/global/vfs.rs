use async_channel::{Receiver, Sender};
use std::num::NonZeroUsize;
use std::sync::Arc;

use tracing::{error, warn};

use crate::allocator::{Allocator, Impl, Slice};
use crate::parser::{NfsArgWrapper, NfsArguments};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::{self, NfsRes, Vfs};

/// One queued NFS procedure: parsed arguments and a channel to send the result.
pub type VfsCommand = (NfsArgWrapper, Sender<ProcReply>);
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
        allocator: Arc<Impl>,
    ) -> Self {
        let (tx, rx) = async_channel::unbounded::<VfsCommand>();

        (0..num.get()).for_each(|_| {
            let rx_clone = rx.clone();
            VfsTask::new(Arc::clone(&backend), Arc::clone(&allocator), rx_clone).spawn();
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
    /// Allocator used for read buffers.
    allocator: Arc<Impl>,
    /// Shared receiver from the pool, each worker competes for the same command stream.
    command_receiver: VfsCommandReceiver,
}

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
        allocator: Arc<Impl>,
        command_receiver: VfsCommandReceiver,
    ) -> Self {
        Self { backend, allocator, command_receiver }
    }

    /// Spawns a [`VfsTask`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio-uring runtime context.
    pub fn spawn(self) {
        tokio_uring::spawn(async move { self.run().await });
    }

    /// Consumes commands until the channel closes, dispatching each NFS op and sending replies.
    async fn run(self) {
        let command_receiver = self.command_receiver;

        while let Ok((command, tx)) = command_receiver.recv().await {
            let NfsArgWrapper { header, proc } = command;
            let proc_name = Self::proc_name(&proc);

            let response = match *proc {
                NfsArguments::Null => NfsRes::Null,
                NfsArguments::GetAttr(args) => NfsRes::GetAttr(self.backend.get_attr(args).await),
                NfsArguments::SetAttr(args) => NfsRes::SetAttr(self.backend.set_attr(args).await),
                NfsArguments::LookUp(args) => NfsRes::LookUp(self.backend.lookup(args).await),
                NfsArguments::Access(args) => NfsRes::Access(self.backend.access(args).await),
                NfsArguments::ReadLink(args) => {
                    NfsRes::ReadLink(self.backend.read_link(args).await)
                }
                NfsArguments::Read(args) => {
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
                        Ok(data) => NfsRes::Read(self.backend.read(args, data).await),
                        Err(err) => NfsRes::Read(Err(err)),
                    }
                }
                NfsArguments::Write(args) => NfsRes::Write(self.backend.write(args).await),
                NfsArguments::Create(args) => NfsRes::Create(self.backend.create(args).await),
                NfsArguments::MkDir(args) => NfsRes::MkDir(self.backend.mk_dir(args).await),
                NfsArguments::SymLink(args) => NfsRes::SymLink(self.backend.symlink(args).await),
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
            if tx.send(reply).await.is_err() {
                warn!("writer task closed, connection pipeline is done");
            }
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
