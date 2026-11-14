#[cfg(test)]
mod tests;

use std::sync::Arc;

use drr_channel::{Cost, Receiver};
use tracing::{error, warn};

use crate::allocator::Buffer;
use crate::backend::BackendRegistry;
use crate::consts::connection::VFS_QUANTUM;
use crate::limits::budget::ReplySender;
use crate::limits::Limits;
use crate::parser::{NfsArgWrapper, NfsArguments};
use crate::rpc::auth::Credential;
use crate::task::{ProcReply, ProcResult};
use crate::vfs::file::BackendId;
use crate::vfs::{self, NfsRes, Vfs};

/// Bytes of READ or WRITE data that cost as much as one procedure.
const COST_UNIT: u32 = 64 * 1024;

/// One queued NFS procedure.
pub struct VfsCommand<B: Buffer> {
    /// Parsed RPC header and procedure arguments.
    pub args: NfsArgWrapper<B>,
    /// Channel to the write task of the connection the reply goes to.
    pub result_tx: ReplySender<B>,
}

impl<B: Buffer> Cost for VfsCommand<B> {
    fn cost(&self) -> u32 {
        cost(&self.args.proc)
    }
}

/// Returns the cost of `proc`: one unit, plus one per started [`COST_UNIT`] of READ or
/// WRITE data.
fn cost<B: Buffer>(proc: &NfsArguments<B>) -> u32 {
    let bytes = match proc {
        NfsArguments::Read(args, _) => args.count,
        NfsArguments::Write(args) => args.size,
        _ => 0,
    };
    1 + bytes.div_ceil(COST_UNIT)
}

/// Queues the procedures of one connection for the pool, see [`VfsManager::sender`].
pub type VfsSender<B> = drr_channel::FlowSender<VfsCommand<B>>;

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

/// Pool of [`VfsTask`] workers fed from one queue per connection, served in deficit
/// round robin, see [`drr_channel`].
///
/// Every worker is a separate tokio task that executes one procedure at a time, so:
/// - procedures run in parallel on all runtime worker threads: the CPU work done inside
///   backend futures (copying data, checksums, synchronous syscalls, ...) is spread over
///   the cores instead of being serialized in a single task;
/// - at most as many procedures as there are workers are executed against the backends
///   at the same time, the rest wait in their queues. The pool size is therefore the
///   backend queue depth.
///
/// Dropping the manager closes the queues; the workers stop once they are drained.
pub struct VfsManager<B: Buffer> {
    /// Opens the queues of new connections.
    sender: drr_channel::Sender<VfsCommand<B>>,
}

impl<B: Buffer + 'static> VfsManager<B> {
    /// Creates a new [`VfsManager`] with [`Limits::vfs_concurrency`] workers that hold at
    /// most [`Limits::max_pending`] procedures.
    ///
    /// # Parameters
    ///
    /// - `backends` --- registry of filesystem implementations, may be empty
    /// - `limits` --- limits of the server
    ///
    /// # Returns
    ///
    /// A new [`VfsManager`] whose workers are already running.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn new<V>(backends: BackendRegistry<V>, limits: &Limits) -> Self
    where
        V: Vfs<B> + Send + Sync + 'static,
    {
        let (sender, receiver) = drr_channel::channel(limits.max_pending, VFS_QUANTUM);
        for _ in 0..limits.vfs_concurrency.get() {
            VfsTask::new(backends.clone(), receiver.clone()).spawn();
        }

        Self { sender }
    }

    /// Opens the queue a new connection sends its procedures through.
    pub fn sender(&self) -> VfsSender<B> {
        self.sender.flow()
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
    /// Queues of the pool, shared by all workers.
    receiver: Receiver<VfsCommand<B>>,
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
    /// - `receiver` --- queues of the pool
    ///
    /// # Returns
    ///
    /// A new [`VfsTask`] that reads commands from the pool and executes them.
    pub fn new(backends: BackendRegistry<V>, receiver: Receiver<VfsCommand<B>>) -> Self {
        Self { backends, receiver }
    }

    /// Spawns a [`VfsTask`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    /// Consumes commands until the pool is closed and drained, dispatching each NFS op
    /// and sending replies.
    async fn run(self) {
        while let Some((command, slot)) = self.receiver.recv().await {
            dispatch(&self.backends, command).await;
            // Held until the reply is queued, then goes to the next procedure.
            drop(slot);
        }
    }
}

/// Routes a single NFS procedure to the backend owning its file handle and sends the reply.
async fn dispatch<V, B>(backends: &BackendRegistry<V>, command: VfsCommand<B>)
where
    B: Buffer + 'static,
    V: vfs::Vfs<B> + Send + Sync + 'static,
{
    let VfsCommand { args: NfsArgWrapper { header, proc }, result_tx } = command;
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
    if result_tx.send(reply).await.is_err() {
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
