use async_channel::{Receiver, Sender};
use std::sync::Arc;

use futures_util::stream::{FuturesUnordered, StreamExt};
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
/// [`FuturesUnordered`].
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
    ///
    /// # Returns
    ///
    /// Creates new [`VfsTask`] and link to it that executes commands.
    pub fn new<V>(backend: Arc<V>) -> Self
    where
        V: Vfs<B> + Send + Sync + 'static,
    {
        let (tx, rx) = async_channel::unbounded::<VfsCommand<B>>();

        VfsTask::new(backend, rx).spawn();

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
/// All received commands are pushed into a [`FuturesUnordered`] and polled concurrently.
pub struct VfsTask<V, B>
where
    B: Buffer,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    /// Shared filesystem implementation.
    backend: Arc<V>,
    /// Receiver from the vfs task, consumed by this single task.
    command_receiver: VfsCommandReceiver<B>,
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
    ///
    /// # Returns
    ///
    /// A new [`VfsTask`] that reads commands from the pool and executes them.
    pub fn new(backend: Arc<V>, command_receiver: VfsCommandReceiver<B>) -> Self {
        Self { backend, command_receiver }
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

        let mut commands = FuturesUnordered::new();

        loop {
            tokio::select! {
                command = command_receiver.recv() => match command {
                    Ok(command) => {
                        commands.push(dispatch(Arc::clone(&backend), command));
                    }
                    Err(_) => break,
                },
                // Poll completed commands. The guard keeps
                // select! from busy-spinning on an empty set.
                _ = commands.next(), if !commands.is_empty() => {}
            }
        }

        // The task is closed; drain in-flight commands before exiting.
        while commands.next().await.is_some() {}
    }
}

/// Executes a single NFS procedure against the backend and sends the reply.
async fn dispatch<V, B>(backend: Arc<V>, command: VfsCommand<B>)
where
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    let (wrapper, tx) = command;
    let NfsArgWrapper { header, proc } = wrapper;
    let proc_name = proc.get_name();

    let response = match *proc {
        NfsArguments::Null => NfsRes::Null,
        NfsArguments::GetAttr(args) => NfsRes::GetAttr(backend.get_attr(args, &header.cred).await),
        NfsArguments::SetAttr(args) => NfsRes::SetAttr(backend.set_attr(args, &header.cred).await),
        NfsArguments::LookUp(args) => NfsRes::LookUp(backend.lookup(args, &header.cred).await),
        NfsArguments::Access(args) => NfsRes::Access(backend.access(args, &header.cred).await),
        NfsArguments::ReadLink(args) => {
            NfsRes::ReadLink(backend.read_link(args, &header.cred).await)
        }
        NfsArguments::Read(args, data) => {
            NfsRes::Read(backend.read(args, data, &header.cred).await)
        }
        NfsArguments::Write(args) => NfsRes::Write(backend.write(args, &header.cred).await),
        NfsArguments::Create(args) => NfsRes::Create(backend.create(args, &header.cred).await),
        NfsArguments::MkDir(args) => NfsRes::MkDir(backend.mk_dir(args, &header.cred).await),
        NfsArguments::SymLink(args) => NfsRes::SymLink(backend.symlink(args, &header.cred).await),
        NfsArguments::MkNod(args) => NfsRes::MkNod(backend.mk_node(args, &header.cred).await),
        NfsArguments::Remove(args) => NfsRes::Remove(backend.remove(args, &header.cred).await),
        NfsArguments::RmDir(args) => NfsRes::RmDir(backend.rm_dir(args, &header.cred).await),
        NfsArguments::Rename(args) => NfsRes::Rename(backend.rename(args, &header.cred).await),
        NfsArguments::Link(args) => NfsRes::Link(backend.link(args, &header.cred).await),
        NfsArguments::ReadDir(args) => NfsRes::ReadDir(backend.read_dir(args, &header.cred).await),
        NfsArguments::ReadDirPlus(args) => {
            NfsRes::ReadDirPlus(backend.read_dir_plus(args, &header.cred).await)
        }
        NfsArguments::FsStat(args) => NfsRes::FsStat(backend.fs_stat(args, &header.cred).await),
        NfsArguments::FsInfo(args) => NfsRes::FsInfo(backend.fs_info(args, &header.cred).await),
        NfsArguments::PathConf(args) => {
            NfsRes::PathConf(backend.path_conf(args, &header.cred).await)
        }
        NfsArguments::Commit(args) => NfsRes::Commit(backend.commit(args, &header.cred).await),
    };

    if let Some(error) = response.error_from_response() {
        error!(xid=header.xid, proc=proc_name, error=?error, "nfs op failed");
    }

    let reply =
        ProcReply { xid: header.xid, proc_result: Ok(ProcResult::Nfs3(Box::new(response))) };

    // Write task may already be closed; then this connection pipeline is done.
    if tx.send(reply).await.is_err() {
        warn!("writer task closed, connection pipeline is done");
    }
}
