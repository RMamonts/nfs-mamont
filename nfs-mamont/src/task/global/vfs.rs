use std::future::Future;
use std::num::NonZeroUsize;
use std::ops::{Sub, SubAssign};
use std::sync::Arc;

use async_channel::{Receiver, Sender, TryRecvError};
use futures_buffered::FuturesUnorderedBounded;
use futures_util::StreamExt;
use tokio::task::yield_now;
use tracing::{error, warn};

use crate::allocator::Buffer;
use crate::parser::{NfsArgWrapper, NfsArguments};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::{self, NfsRes, Vfs};

/// One queued NFS procedure: parsed arguments and a channel to send the result.
pub type VfsCommand<B> = (NfsArgWrapper<B>, Sender<ProcReply<B>>);
/// Sender to enqueue work in the queue.
pub type VfsCommandSender<B> = Sender<VfsCommand<B>>;
/// Receiver from the queue, consumed by the single VFS task.
type VfsCommandReceiver<B> = Receiver<VfsCommand<B>>;

/// Manager that contains only one task that dispatching NFS procedures to a single task running them concurrently via
/// [`FuturesUnorderedBounded`].
pub struct VfsManager<B: Buffer> {
    /// Sender to enqueue work in the vfs task for execution.
    sender: VfsCommandSender<B>,
}

impl<B: Buffer + 'static> VfsManager<B> {
    /// Creates a new [`VfsManager`] backed by a single task.
    ///
    /// # Parameters
    ///
    /// - `backend` --- shared filesystem implementation
    /// - `vfs_concurrency` --- max concurrent VFS operations
    ///
    /// # Returns
    ///
    /// Creates new [`VfsTask`] and link to it that executes commands.
    pub fn new<V>(backend: Arc<V>, vfs_concurrency: NonZeroUsize) -> Self
    where
        V: Vfs<B> + Send + Sync + 'static,
    {
        let (tx, rx) = async_channel::unbounded::<VfsCommand<B>>();

        VfsTask::new(backend, rx, vfs_concurrency).spawn();

        Self { sender: tx }
    }

    /// Returns a clone of the command sender for enqueueing work in the queue.
    pub fn sender(&self) -> VfsCommandSender<B> {
        self.sender.clone()
    }
}

impl<B: Buffer> Drop for VfsManager<B> {
    /// Closes the [`VfsTask`] sender so the task stops after the command stream is empty.
    fn drop(&mut self) {
        self.sender.close();
    }
}

/// Task that executes NFS procedures against [`Vfs`] and sends the result to the writer pipeline.
///
/// All received commands are pushed into a [`FuturesUnorderedBounded`] and polled concurrently.
pub struct VfsTask<V, B>
where
    B: Buffer,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    /// Shared filesystem implementation.
    backend: Arc<V>,
    /// Receiver from the vfs task, consumed by this single task.
    command_receiver: VfsCommandReceiver<B>,
    /// Maximum number of concurrent VFS operations.
    vfs_concurrency: NonZeroUsize,
}

impl<V, B> VfsTask<V, B>
where
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    /// Builds a task that reads commands from the channel and executes them.
    ///
    /// # Parameters
    ///
    /// - `backend` --- shared filesystem implementation
    /// - `command_receiver` --- receiver from the read task
    /// - `vfs_concurrency` --- max concurrent VFS operations
    ///
    /// # Returns
    ///
    /// A new [`VfsTask`] that reads commands from the pool and executes them.
    pub fn new(
        backend: Arc<V>,
        command_receiver: VfsCommandReceiver<B>,
        vfs_concurrency: NonZeroUsize,
    ) -> Self {
        Self { backend, command_receiver, vfs_concurrency }
    }

    /// Spawns a [`VfsTask`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    /// Consumes commands until the channel closes, executing each NFS op concurrently
    /// and sending replies.
    async fn run(self) {
        let backend = self.backend;
        let command_receiver = self.command_receiver;
        let vfs_concurrency = self.vfs_concurrency.get();

        let mut commands = FuturesUnorderedBounded::new(vfs_concurrency);
        let mut capacity_left = vfs_concurrency;
        let mut finish = false;

        while !finish {
            loop {
                match command_receiver.try_recv() {
                    Ok(command) => {
                        capacity_left -= 1;
                        let fut = dispatch(Arc::clone(&backend), command);
                        if commands.try_push(fut).is_err() {
                            unreachable!(
                                "since there is explicit counter, there could not be push fail"
                            )
                        }
                        if capacity_left == 0 {
                            break;
                        }
                    }
                    Err(TryRecvError::Closed) => {
                        finish = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => {
                        break;
                    }
                }
            }
            while commands.next().await.is_some() {
                capacity_left += 1;
                match command_receiver.try_recv() {
                    Ok(command) => {
                        capacity_left -= 1;
                        let fut = dispatch(Arc::clone(&backend), command);
                        if commands.try_push(fut).is_err() {
                            unreachable!(
                                "since there is explicit counter, there could not be push fail"
                            )
                        }
                    }
                    // ignore errors, since futures still must be processed
                    _ => ()
                }
            }
            // in case there are no futures to process in FuturesUnordered and channel is empty
            if capacity_left == vfs_concurrency {
                yield_now().await;
            }
        }
    }
}

/// Executes a single NFS procedure against the backend and sends the reply.
async fn dispatch<V, B>(backend: Arc<V>, command: VfsCommand<B>)
where
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    let (args, tx) = command;
    let proc_name = args.proc.get_name();

    let response = match *args.proc {
        NfsArguments::Null => NfsRes::Null,
        NfsArguments::GetAttr(args) => NfsRes::GetAttr(backend.get_attr(args).await),
        NfsArguments::SetAttr(args) => NfsRes::SetAttr(backend.set_attr(args).await),
        NfsArguments::LookUp(args) => NfsRes::LookUp(backend.lookup(args).await),
        NfsArguments::Access(args) => NfsRes::Access(backend.access(args).await),
        NfsArguments::ReadLink(args) => NfsRes::ReadLink(backend.read_link(args).await),
        NfsArguments::Read(args, data) => NfsRes::Read(backend.read(args, data).await),
        NfsArguments::Write(args) => NfsRes::Write(backend.write(args).await),
        NfsArguments::Create(args) => NfsRes::Create(backend.create(args).await),
        NfsArguments::MkDir(args) => NfsRes::MkDir(backend.mk_dir(args).await),
        NfsArguments::SymLink(args) => NfsRes::SymLink(backend.symlink(args).await),
        NfsArguments::MkNod(args) => NfsRes::MkNod(backend.mk_node(args).await),
        NfsArguments::Remove(args) => NfsRes::Remove(backend.remove(args).await),
        NfsArguments::RmDir(args) => NfsRes::RmDir(backend.rm_dir(args).await),
        NfsArguments::Rename(args) => NfsRes::Rename(backend.rename(args).await),
        NfsArguments::Link(args) => NfsRes::Link(backend.link(args).await),
        NfsArguments::ReadDir(args) => NfsRes::ReadDir(backend.read_dir(args).await),
        NfsArguments::ReadDirPlus(args) => NfsRes::ReadDirPlus(backend.read_dir_plus(args).await),
        NfsArguments::FsStat(args) => NfsRes::FsStat(backend.fs_stat(args).await),
        NfsArguments::FsInfo(args) => NfsRes::FsInfo(backend.fs_info(args).await),
        NfsArguments::PathConf(args) => NfsRes::PathConf(backend.path_conf(args).await),
        NfsArguments::Commit(args) => NfsRes::Commit(backend.commit(args).await),
    };

    if let Some(error) = response.error_from_response() {
        error!(xid=args.header.xid, proc=proc_name, error=?error, "nfs op failed");
    }

    let reply =
        ProcReply { xid: args.header.xid, proc_result: Ok(ProcResult::Nfs3(Box::new(response))) };

    // Write task may already be closed; then this connection pipeline is done.
    if tx.send(reply).await.is_err() {
        warn!("writer task closed, connection pipeline is done");
    }
}
