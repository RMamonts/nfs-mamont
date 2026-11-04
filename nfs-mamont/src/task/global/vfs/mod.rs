pub mod scheduler;
#[cfg(test)]
mod tests;

use async_channel::{Receiver, SendError, Sender};
use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::sync::OwnedSemaphorePermit;
use tracing::{error, warn};

use crate::allocator::Buffer;
use crate::backend::BackendRegistry;
use crate::limits::Scheduling;
use crate::parser::{NfsArgWrapper, NfsArguments};
use crate::rpc::auth::Credential;
use crate::task::connection::limit::Token;
use crate::task::global::vfs::scheduler::{FlowHandle, Scheduler};
use crate::task::{InFlightReply, ProcReply, ProcResult};
use crate::vfs::file::BackendId;
use crate::vfs::{self, NfsRes, Vfs};

/// One queued NFS procedure.
pub struct VfsCommand<B: Buffer> {
    /// Parsed RPC header and procedure arguments.
    pub args: NfsArgWrapper<B>,
    /// Channel to the write task of the connection the reply goes to.
    pub result_tx: Sender<InFlightReply<B>>,
    /// In-flight slot of the request, returned by the write task after the reply is
    /// written, see [`crate::task::connection`].
    pub token: Token,
    /// Slot of the request in the global pending budget, if one is set; given back
    /// once the procedure is finished, see [`crate::limits`].
    pub pending: Option<OwnedSemaphorePermit>,
}

/// Where connections queue procedures for the pool.
enum Queue<B: Buffer> {
    /// One shared channel served in arrival order.
    Fifo(Sender<VfsCommand<B>>),
    /// A queue per connection served by deficit round robin.
    Drr(Arc<Scheduler<B>>),
}

/// Handle of one connection to queue procedures in the pool.
pub enum FlowSender<B: Buffer> {
    /// Sender of the shared channel.
    Fifo(Sender<VfsCommand<B>>),
    /// Queue of the connection in the scheduler.
    Drr(FlowHandle<B>),
}

impl<B: Buffer> FlowSender<B> {
    /// Queues `command` for execution.
    ///
    /// # Errors
    ///
    /// Gives the command back when the pool is gone.
    // The command is returned on purpose, so that the caller can answer it.
    #[allow(clippy::result_large_err)]
    pub async fn send(&self, command: VfsCommand<B>) -> Result<(), VfsCommand<B>> {
        match self {
            Self::Fifo(sender) => sender.send(command).await.map_err(|SendError(command)| command),
            Self::Drr(flow) => flow.enqueue(command),
        }
    }
}

/// Where the workers take procedures from.
enum Source<B: Buffer> {
    Fifo(Receiver<VfsCommand<B>>),
    Drr(Arc<Scheduler<B>>),
}

impl<B: Buffer> Clone for Source<B> {
    fn clone(&self) -> Self {
        match self {
            Self::Fifo(receiver) => Self::Fifo(receiver.clone()),
            Self::Drr(scheduler) => Self::Drr(Arc::clone(scheduler)),
        }
    }
}

impl<B: Buffer> Source<B> {
    /// Takes the next procedure, waiting while none is queued.
    ///
    /// # Returns
    ///
    /// `None` once the pool is closed and drained.
    async fn next(&self) -> Option<VfsCommand<B>> {
        match self {
            Self::Fifo(receiver) => receiver.recv().await.ok(),
            Self::Drr(scheduler) => scheduler.next().await,
        }
    }
}

/// Backend a parsed procedure has to be executed against.
enum Target {
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
fn target<B: Buffer>(proc: &NfsArguments<B>) -> Target {
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
fn failed_response<B: Buffer>(proc: &NfsArguments<B>, error: vfs::Error) -> NfsRes<B> {
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

/// Pool of [`VfsTask`] workers fed from a shared queue.
///
/// The queue is unbounded, but every queued command holds an in-flight token of its
/// connection (see [`crate::task::connection`]) and, when one is set, a slot of the
/// global pending budget (see [`crate::limits`]), so it never holds more than
/// `max_in_flight × open connections` or `max_pending` commands. With
/// [`Scheduling::Drr`] every connection has its own queue and the workers take
/// procedures in deficit round robin, see [`scheduler`].
///
/// Every worker is a separate tokio task that executes one procedure at a time, so:
/// - procedures run in parallel on all runtime worker threads: the CPU work done inside
///   backend futures (copying data, checksums, synchronous syscalls, ...) is spread over
///   the cores instead of being serialized in a single task;
/// - at most as many procedures as there are workers are executed against the backends
///   at the same time, the rest wait in the channel. The pool size is therefore the
///   backend queue depth.
pub struct VfsManager<B: Buffer> {
    /// Where connections queue work for the pool.
    queue: Queue<B>,
}

impl<B: Buffer + 'static> VfsManager<B> {
    /// Creates a new [`VfsManager`] with `concurrency` workers.
    ///
    /// # Parameters
    ///
    /// - `backends` --- registry of filesystem implementations, may be empty
    /// - `concurrency` --- number of workers, i.e. the maximum number of procedures
    ///   executed against the backends at the same time
    /// - `scheduling` --- how the workers pick the next procedure
    ///
    /// # Returns
    ///
    /// A new [`VfsManager`] whose workers are already running.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn new<V>(
        backends: BackendRegistry<V>,
        concurrency: NonZeroUsize,
        scheduling: Scheduling,
    ) -> Self
    where
        V: Vfs<B> + Send + Sync + 'static,
    {
        let (queue, source) = match scheduling {
            Scheduling::Fifo => {
                let (tx, rx) = async_channel::unbounded::<VfsCommand<B>>();
                (Queue::Fifo(tx), Source::Fifo(rx))
            }
            Scheduling::Drr { quantum } => {
                let scheduler = Scheduler::new(quantum);
                (Queue::Drr(Arc::clone(&scheduler)), Source::Drr(scheduler))
            }
        };

        for _ in 0..concurrency.get() {
            VfsTask::new(backends.clone(), source.clone()).spawn();
        }

        Self { queue }
    }

    /// Returns the handle a new connection queues its procedures through.
    pub fn flow(&self) -> FlowSender<B> {
        match &self.queue {
            Queue::Fifo(sender) => FlowSender::Fifo(sender.clone()),
            Queue::Drr(scheduler) => FlowSender::Drr(scheduler.register()),
        }
    }
}

impl<B: Buffer> Drop for VfsManager<B> {
    /// Closes the pool's queue so workers stop after it is empty.
    fn drop(&mut self) {
        match &self.queue {
            Queue::Fifo(sender) => {
                sender.close();
            }
            Queue::Drr(scheduler) => scheduler.close(),
        }
    }
}

/// Worker of [`VfsManager`]: executes NFS procedures against [`Vfs`] one at a time and
/// sends the result to the writer pipeline.
pub struct VfsTask<V, B>
where
    B: Buffer,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    /// Registry of filesystem implementations, indexed by the first byte of a file handle.
    backends: BackendRegistry<V>,
    /// Shared queue of the pool, each worker competes for the same command stream.
    source: Source<B>,
}

impl<V, B> VfsTask<V, B>
where
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    /// Builds a worker that reads commands from the pool and executes them.
    ///
    /// # Parameters
    ///
    /// - `backends` --- registry of filesystem implementations, may be empty
    /// - `source` --- shared queue of the pool
    ///
    /// # Returns
    ///
    /// A new [`VfsTask`] that reads commands from the pool and executes them.
    fn new(backends: BackendRegistry<V>, source: Source<B>) -> Self {
        Self { backends, source }
    }

    /// Spawns a [`VfsTask`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    /// Consumes commands until the queue is closed and empty, dispatching each NFS op
    /// and sending replies.
    async fn run(self) {
        while let Some(command) = self.source.next().await {
            dispatch(&self.backends, command).await;
        }
    }
}

/// Routes a single NFS procedure to the backend owning its file handle and sends the reply.
async fn dispatch<V, B>(backends: &BackendRegistry<V>, command: VfsCommand<B>)
where
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    let VfsCommand { args: NfsArgWrapper { header, proc }, result_tx, token, pending } = command;
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
                    backend_slot = backend_id.slot(),
                    backend_generation = backend_id.generation(),
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
    if result_tx.send(InFlightReply { reply, token }).await.is_err() {
        warn!("writer task closed, connection pipeline is done");
    }
    // The procedure is finished: its slot of the pending budget goes to the next
    // request, while the in-flight slot of the connection stays taken until the
    // reply is written.
    drop(pending);
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
