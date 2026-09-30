use async_channel::{Receiver, Sender};
use std::sync::Arc;

use futures_util::stream::{FuturesUnordered, StreamExt};
use tracing::{error, warn};

use crate::allocator::Buffer;
use crate::backend::BackendRegistry;
use crate::parser::{NfsArgWrapper, NfsArguments};
use crate::rpc::auth::Credential;
use crate::task::{ProcReply, ProcResult};
use crate::vfs::file::BackendId;
use crate::vfs::{self, NfsRes, Vfs};

/// One queued NFS procedure: parsed arguments and a channel to send the result.
pub type VfsCommand<B> = (NfsArgWrapper<B>, Sender<ProcReply<B>>);
/// Sender to enqueue work in the queue.
pub type VfsCommandSender<B> = Sender<VfsCommand<B>>;
/// Receiver from the queue, consumed by the single VFS task.
type VfsCommandReceiver<B> = Receiver<VfsCommand<B>>;

/// Backend a parsed procedure has to be executed against.
pub enum Target {
    /// The procedure does not address any object, so no backend is needed.
    None,
    /// The procedure addresses objects of a single backend.
    Single(BackendId),
    /// The procedure addresses objects of two different backends, which is unsupported.
    Crossing,
}

/// Determines which backend has to serve the procedure.
///
/// The backend index is taken from the first byte of the file handle carried by the
/// arguments. Procedures addressing two objects must keep both of them within the
/// same backend, otherwise [`Target::Crossing`] is returned.
pub fn target<B: Buffer>(proc: &NfsArguments<B>) -> Target {
    let (first, second) = match proc {
        NfsArguments::Null => return Target::None,
        NfsArguments::GetAttr(args) => (&args.file, None),
        NfsArguments::SetAttr(args) => (&args.file, None),
        NfsArguments::LookUp(args) => (&args.parent, None),
        NfsArguments::Access(args) => (&args.file, None),
        NfsArguments::ReadLink(args) => (&args.file, None),
        NfsArguments::Read(args, _) => (&args.file, None),
        NfsArguments::Write(args) => (&args.file, None),
        NfsArguments::Create(args) => (&args.object.dir, None),
        NfsArguments::MkDir(args) => (&args.object.dir, None),
        NfsArguments::SymLink(args) => (&args.object.dir, None),
        NfsArguments::MkNod(args) => (&args.object.dir, None),
        NfsArguments::Remove(args) => (&args.object.dir, None),
        NfsArguments::RmDir(args) => (&args.object.dir, None),
        NfsArguments::Rename(args) => (&args.from.dir, Some(&args.to.dir)),
        NfsArguments::Link(args) => (&args.file, Some(&args.link.dir)),
        NfsArguments::ReadDir(args) => (&args.dir, None),
        NfsArguments::ReadDirPlus(args) => (&args.dir, None),
        NfsArguments::FsStat(args) => (&args.root, None),
        NfsArguments::FsInfo(args) => (&args.root, None),
        NfsArguments::PathConf(args) => (&args.file, None),
        NfsArguments::Commit(args) => (&args.file, None),
    };

    match second {
        Some(second) if second.backend_id() != first.backend_id() => Target::Crossing,
        _ => Target::Single(first.backend_id()),
    }
}

/// Builds a failed [`NfsRes`] carrying `error` for the given procedure variant.
///
/// Used when the procedure cannot reach a backend at all, so no backend-provided
/// attributes are available.
pub fn failed_response<B: Buffer>(proc: &NfsArguments<B>, error: vfs::Error) -> NfsRes<B> {
    let wcc_data = || vfs::WccData { before: None, after: None };

    match proc {
        NfsArguments::Null => NfsRes::Null,
        NfsArguments::GetAttr(_) => NfsRes::GetAttr(Err(vfs::get_attr::Fail { error })),
        NfsArguments::SetAttr(_) => {
            NfsRes::SetAttr(Err(vfs::set_attr::Fail { error, wcc_data: wcc_data() }))
        }
        NfsArguments::LookUp(_) => NfsRes::LookUp(Err(vfs::lookup::Fail { error, dir_attr: None })),
        NfsArguments::Access(_) => {
            NfsRes::Access(Err(vfs::access::Fail { error, object_attr: None }))
        }
        NfsArguments::ReadLink(_) => {
            NfsRes::ReadLink(Err(vfs::read_link::Fail { error, symlink_attr: None }))
        }
        NfsArguments::Read(..) => NfsRes::Read(Err(vfs::read::Fail { error, file_attr: None })),
        NfsArguments::Write(_) => {
            NfsRes::Write(Err(vfs::write::Fail { error, wcc_data: wcc_data() }))
        }
        NfsArguments::Create(_) => {
            NfsRes::Create(Err(vfs::create::Fail { error, wcc_data: wcc_data() }))
        }
        NfsArguments::MkDir(_) => {
            NfsRes::MkDir(Err(vfs::mk_dir::Fail { error, dir_wcc: wcc_data() }))
        }
        NfsArguments::SymLink(_) => {
            NfsRes::SymLink(Err(vfs::symlink::Fail { error, dir_wcc: wcc_data() }))
        }
        NfsArguments::MkNod(_) => {
            NfsRes::MkNod(Err(vfs::mk_node::Fail { error, dir_wcc: wcc_data() }))
        }
        NfsArguments::Remove(_) => {
            NfsRes::Remove(Err(vfs::remove::Fail { error, dir_wcc: wcc_data() }))
        }
        NfsArguments::RmDir(_) => {
            NfsRes::RmDir(Err(vfs::rm_dir::Fail { error, dir_wcc: wcc_data() }))
        }
        NfsArguments::Rename(_) => NfsRes::Rename(Err(vfs::rename::Fail {
            error,
            from_dir_wcc: wcc_data(),
            to_dir_wcc: wcc_data(),
        })),
        NfsArguments::Link(_) => {
            NfsRes::Link(Err(vfs::link::Fail { error, file_attr: None, dir_wcc: wcc_data() }))
        }
        NfsArguments::ReadDir(_) => {
            NfsRes::ReadDir(Err(vfs::read_dir::Fail { error, dir_attr: None }))
        }
        NfsArguments::ReadDirPlus(_) => {
            NfsRes::ReadDirPlus(Err(vfs::read_dir_plus::Fail { error, dir_attr: None }))
        }
        NfsArguments::FsStat(_) => {
            NfsRes::FsStat(Err(vfs::fs_stat::Fail { error, root_attr: None }))
        }
        NfsArguments::FsInfo(_) => {
            NfsRes::FsInfo(Err(vfs::fs_info::Fail { error, root_attr: None }))
        }
        NfsArguments::PathConf(_) => {
            NfsRes::PathConf(Err(vfs::path_conf::Fail { error, file_attr: None }))
        }
        NfsArguments::Commit(_) => {
            NfsRes::Commit(Err(vfs::commit::Fail { error, file_wcc: wcc_data() }))
        }
    }
}

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
    /// - `backends` --- registry of filesystem implementations, may be empty
    ///
    /// # Returns
    ///
    /// Creates new [`VfsTask`] and link to it that executes commands.
    pub fn new<V>(backends: BackendRegistry<V>) -> Self
    where
        V: Vfs<B> + Send + Sync + 'static,
    {
        let (tx, rx) = async_channel::unbounded::<VfsCommand<B>>();

        VfsTask::new(backends, rx).spawn();

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
    /// Registry of filesystem implementations, indexed by the first byte of a file handle.
    backends: BackendRegistry<V>,
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
    /// - `backends` --- registry of filesystem implementations, may be empty
    /// - `command_receiver` --- receiver from the read task
    ///
    /// # Returns
    ///
    /// A new [`VfsTask`] that reads commands from the pool and executes them.
    pub fn new(backends: BackendRegistry<V>, command_receiver: VfsCommandReceiver<B>) -> Self {
        Self { backends, command_receiver }
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
        let backends = self.backends;
        let command_receiver = self.command_receiver;

        let mut commands = FuturesUnordered::new();

        loop {
            tokio::select! {
                command = command_receiver.recv() => match command {
                    Ok(command) => {
                        commands.push(dispatch(backends.clone(), command));
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

/// Routes a single NFS procedure to the backend owning its file handle and sends the reply.
async fn dispatch<V, B>(backends: BackendRegistry<V>, command: VfsCommand<B>)
where
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    let (wrapper, tx) = command;
    let NfsArgWrapper { header, proc } = wrapper;
    let proc_name = proc.get_name();

    let response = match target(&proc) {
        Target::None => NfsRes::Null,
        Target::Crossing => failed_response(&proc, vfs::Error::XDev),
        Target::Single(backend_id) => match backends.get(backend_id) {
            Some(backend) => execute(*proc, backend, &header.cred).await,
            None => {
                warn!(
                    xid = header.xid,
                    proc = %proc_name,
                    backend = backend_id,
                    "no backend attached for this file handle"
                );

                failed_response(&proc, vfs::Error::StaleFile)
            }
        },
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

/// Runs the procedure against `backend` and wraps its result.
async fn execute<V, B>(proc: NfsArguments<B>, backend: Arc<V>, cred: &Credential) -> NfsRes<B>
where
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    match proc {
        NfsArguments::Null => NfsRes::Null,
        NfsArguments::GetAttr(args) => NfsRes::GetAttr(backend.get_attr(args, cred).await),
        NfsArguments::SetAttr(args) => NfsRes::SetAttr(backend.set_attr(args, cred).await),
        NfsArguments::LookUp(args) => NfsRes::LookUp(backend.lookup(args, cred).await),
        NfsArguments::Access(args) => NfsRes::Access(backend.access(args, cred).await),
        NfsArguments::ReadLink(args) => NfsRes::ReadLink(backend.read_link(args, cred).await),
        NfsArguments::Read(args, data) => NfsRes::Read(backend.read(args, data, cred).await),
        NfsArguments::Write(args) => NfsRes::Write(backend.write(args, cred).await),
        NfsArguments::Create(args) => NfsRes::Create(backend.create(args, cred).await),
        NfsArguments::MkDir(args) => NfsRes::MkDir(backend.mk_dir(args, cred).await),
        NfsArguments::SymLink(args) => NfsRes::SymLink(backend.symlink(args, cred).await),
        NfsArguments::MkNod(args) => NfsRes::MkNod(backend.mk_node(args, cred).await),
        NfsArguments::Remove(args) => NfsRes::Remove(backend.remove(args, cred).await),
        NfsArguments::RmDir(args) => NfsRes::RmDir(backend.rm_dir(args, cred).await),
        NfsArguments::Rename(args) => NfsRes::Rename(backend.rename(args, cred).await),
        NfsArguments::Link(args) => NfsRes::Link(backend.link(args, cred).await),
        NfsArguments::ReadDir(args) => NfsRes::ReadDir(backend.read_dir(args, cred).await),
        NfsArguments::ReadDirPlus(args) => {
            NfsRes::ReadDirPlus(backend.read_dir_plus(args, cred).await)
        }
        NfsArguments::FsStat(args) => NfsRes::FsStat(backend.fs_stat(args, cred).await),
        NfsArguments::FsInfo(args) => NfsRes::FsInfo(backend.fs_info(args, cred).await),
        NfsArguments::PathConf(args) => NfsRes::PathConf(backend.path_conf(args, cred).await),
        NfsArguments::Commit(args) => NfsRes::Commit(backend.commit(args, cred).await),
    }
}
