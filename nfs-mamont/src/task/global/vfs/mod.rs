#[cfg(test)]
mod tests;

use std::marker::PhantomData;
use std::sync::Arc;

use drr_channel::Cost;
use tracing::{error, warn};

use crate::allocator::Buffer;
use crate::backend::BackendRegistry;
use crate::limits::budget::ReplySender;
use crate::limits::Limits;
use crate::parser::{Nfs3ArgWrapper, Nfs3Arguments};
use crate::rpc::auth::Credential;
use crate::task::global::manager::{ManagerArgs, ManagerTaskReceiver};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::nfsv3::file::BackendId;
use crate::vfs::nfsv3::{self, NfsRes, Vfs};

/// Bytes of READ or WRITE data that cost as much as one procedure.
const COST_UNIT: u32 = 64 * 1024;

/// One queued NFSv3 procedure.
pub struct Nfs3Command<B: Buffer> {
    /// Parsed RPC header and procedure arguments.
    pub args: Nfs3ArgWrapper<B>,
    /// Channel to the write task of the connection the reply goes to.
    pub result_tx: ReplySender<B>,
}

impl<B: Buffer> Cost for Nfs3Command<B> {
    fn cost(&self) -> u32 {
        cost(&self.args.proc)
    }
}

/// Returns the cost of `proc`: one unit, plus one per started [`COST_UNIT`] of READ or
/// WRITE data.
fn cost<B: Buffer>(proc: &Nfs3Arguments<B>) -> u32 {
    let bytes = match proc {
        Nfs3Arguments::Read(args, _) => args.count,
        Nfs3Arguments::Write(args) => args.size,
        _ => 0,
    };
    1 + bytes.div_ceil(COST_UNIT)
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
fn target<B: Buffer>(proc: &Nfs3Arguments<B>) -> Target {
    let (first, second) = match proc {
        Nfs3Arguments::Null => return Target::None,
        Nfs3Arguments::GetAttr(args) => (&args.file, None),
        Nfs3Arguments::SetAttr(args) => (&args.file, None),
        Nfs3Arguments::LookUp(args) => (&args.parent, None),
        Nfs3Arguments::Access(args) => (&args.file, None),
        Nfs3Arguments::ReadLink(args) => (&args.file, None),
        Nfs3Arguments::Read(args, _) => (&args.file, None),
        Nfs3Arguments::Write(args) => (&args.file, None),
        Nfs3Arguments::Create(args) => (&args.object.dir, None),
        Nfs3Arguments::MkDir(args) => (&args.object.dir, None),
        Nfs3Arguments::SymLink(args) => (&args.object.dir, None),
        Nfs3Arguments::MkNod(args) => (&args.object.dir, None),
        Nfs3Arguments::Remove(args) => (&args.object.dir, None),
        Nfs3Arguments::RmDir(args) => (&args.object.dir, None),
        Nfs3Arguments::Rename(args) => (&args.from.dir, Some(&args.to.dir)),
        Nfs3Arguments::Link(args) => (&args.file, Some(&args.link.dir)),
        Nfs3Arguments::ReadDir(args) => (&args.dir, None),
        Nfs3Arguments::ReadDirPlus(args) => (&args.dir, None),
        Nfs3Arguments::FsStat(args) => (&args.root, None),
        Nfs3Arguments::FsInfo(args) => (&args.root, None),
        Nfs3Arguments::PathConf(args) => (&args.file, None),
        Nfs3Arguments::Commit(args) => (&args.file, None),
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
fn failed_response<B: Buffer>(proc: &Nfs3Arguments<B>, error: nfsv3::Error) -> NfsRes<B> {
    let wcc_data = || nfsv3::WccData { before: None, after: None };

    match proc {
        Nfs3Arguments::Null => NfsRes::Null,
        Nfs3Arguments::GetAttr(_) => NfsRes::GetAttr(Err(nfsv3::get_attr::Fail { error })),
        Nfs3Arguments::SetAttr(_) => {
            NfsRes::SetAttr(Err(nfsv3::set_attr::Fail { error, wcc_data: wcc_data() }))
        }
        Nfs3Arguments::LookUp(_) => {
            NfsRes::LookUp(Err(nfsv3::lookup::Fail { error, dir_attr: None }))
        }
        Nfs3Arguments::Access(_) => {
            NfsRes::Access(Err(nfsv3::access::Fail { error, object_attr: None }))
        }
        Nfs3Arguments::ReadLink(_) => {
            NfsRes::ReadLink(Err(nfsv3::read_link::Fail { error, symlink_attr: None }))
        }
        Nfs3Arguments::Read(..) => NfsRes::Read(Err(nfsv3::read::Fail { error, file_attr: None })),
        Nfs3Arguments::Write(_) => {
            NfsRes::Write(Err(nfsv3::write::Fail { error, wcc_data: wcc_data() }))
        }
        Nfs3Arguments::Create(_) => {
            NfsRes::Create(Err(nfsv3::create::Fail { error, wcc_data: wcc_data() }))
        }
        Nfs3Arguments::MkDir(_) => {
            NfsRes::MkDir(Err(nfsv3::mk_dir::Fail { error, dir_wcc: wcc_data() }))
        }
        Nfs3Arguments::SymLink(_) => {
            NfsRes::SymLink(Err(nfsv3::symlink::Fail { error, dir_wcc: wcc_data() }))
        }
        Nfs3Arguments::MkNod(_) => {
            NfsRes::MkNod(Err(nfsv3::mk_node::Fail { error, dir_wcc: wcc_data() }))
        }
        Nfs3Arguments::Remove(_) => {
            NfsRes::Remove(Err(nfsv3::remove::Fail { error, dir_wcc: wcc_data() }))
        }
        Nfs3Arguments::RmDir(_) => {
            NfsRes::RmDir(Err(nfsv3::rm_dir::Fail { error, dir_wcc: wcc_data() }))
        }
        Nfs3Arguments::Rename(_) => NfsRes::Rename(Err(nfsv3::rename::Fail {
            error,
            from_dir_wcc: wcc_data(),
            to_dir_wcc: wcc_data(),
        })),
        Nfs3Arguments::Link(_) => {
            NfsRes::Link(Err(nfsv3::link::Fail { error, file_attr: None, dir_wcc: wcc_data() }))
        }
        Nfs3Arguments::ReadDir(_) => {
            NfsRes::ReadDir(Err(nfsv3::read_dir::Fail { error, dir_attr: None }))
        }
        Nfs3Arguments::ReadDirPlus(_) => {
            NfsRes::ReadDirPlus(Err(nfsv3::read_dir_plus::Fail { error, dir_attr: None }))
        }
        Nfs3Arguments::FsStat(_) => {
            NfsRes::FsStat(Err(nfsv3::fs_stat::Fail { error, root_attr: None }))
        }
        Nfs3Arguments::FsInfo(_) => {
            NfsRes::FsInfo(Err(nfsv3::fs_info::Fail { error, root_attr: None }))
        }
        Nfs3Arguments::PathConf(_) => {
            NfsRes::PathConf(Err(nfsv3::path_conf::Fail { error, file_attr: None }))
        }
        Nfs3Arguments::Commit(_) => {
            NfsRes::Commit(Err(nfsv3::commit::Fail { error, file_wcc: wcc_data() }))
        }
    }
}

/// Pool of [`VfsTask`] workers fed from one queue per connection
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
    _inner: PhantomData<B>,
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
    pub fn spawn_vfs_tasks<V>(
        backends: BackendRegistry<V>,
        receiver: ManagerTaskReceiver<B>,
        limits: &Limits,
    ) where
        V: Vfs<B> + Send + Sync + 'static,
    {
        for _ in 0..limits.vfs_concurrency.get() {
            VfsTask::new(backends.clone(), receiver.clone()).spawn();
        }
    }
}

/// Worker of [`VfsManager`]: executes NFS procedures against [`Vfs`] one at a time and
/// sends the result to the writer pipeline.
pub struct VfsTask<V, B>
where
    B: Buffer,
    V: nfsv3::Vfs<B> + Send + Sync + 'static,
{
    /// Registry of filesystem implementations, indexed by the first byte of a file handle.
    backends: BackendRegistry<V>,
    /// Queues of the pool, shared by all workers.
    receiver: ManagerTaskReceiver<B>,
}

impl<V, B> VfsTask<V, B>
where
    B: Buffer + 'static,
    V: nfsv3::Vfs<B> + Send + Sync + 'static,
{
    /// Builds a worker that reads commands from the pool and executes them.
    ///
    /// # Parameters
    ///
    /// - `backends` --- registry of filesystem implementations, may be empty
    /// - `receiver` --- receiver of messages from [`crate::ManagerTask`]
    ///
    /// # Returns
    ///
    /// A new [`VfsTask`] that reads commands from the pool and executes them.
    pub fn new(backends: BackendRegistry<V>, receiver: ManagerTaskReceiver<B>) -> Self {
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
        while let Ok(command) = self.receiver.recv().await {
            // TODO: move to dispatch, when there is actual handle for nfsv4
            match command {
                ManagerArgs::NFSv3(inner) => {
                    dispatch(&self.backends, inner.args, inner.reply_sender).await;
                    // Held until the reply is queued, then goes to the next procedure.
                    drop(inner.slot);
                }
                ManagerArgs::NFSv4(_) => unimplemented!(),
            };
        }
    }
}

/// Routes a single NFS procedure to the backend owning its file handle and sends the reply.
async fn dispatch<V, B>(
    backends: &BackendRegistry<V>,
    command: Nfs3ArgWrapper<B>,
    result_tx: ReplySender<B>,
) where
    B: Buffer + 'static,
    V: nfsv3::Vfs<B> + Send + Sync + 'static,
{
    let Nfs3ArgWrapper { header, proc } = command;
    let proc_name = proc.get_name();

    let response = match target(&proc) {
        Target::None => NfsRes::Null,
        Target::Crossing => failed_response(&proc, nfsv3::Error::XDev),
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

                failed_response(&proc, nfsv3::Error::StaleFile)
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
async fn execute<V, B>(proc: Nfs3Arguments<B>, backend: Arc<V>, cred: &Credential) -> NfsRes<B>
where
    B: Buffer + 'static,
    V: nfsv3::Vfs<B> + Send + Sync + 'static,
{
    match proc {
        Nfs3Arguments::Null => NfsRes::Null,
        Nfs3Arguments::GetAttr(args) => NfsRes::GetAttr(backend.get_attr(args, cred).await),
        Nfs3Arguments::SetAttr(args) => NfsRes::SetAttr(backend.set_attr(args, cred).await),
        Nfs3Arguments::LookUp(args) => NfsRes::LookUp(backend.lookup(args, cred).await),
        Nfs3Arguments::Access(args) => NfsRes::Access(backend.access(args, cred).await),
        Nfs3Arguments::ReadLink(args) => NfsRes::ReadLink(backend.read_link(args, cred).await),
        Nfs3Arguments::Read(args, data) => NfsRes::Read(backend.read(args, data, cred).await),
        Nfs3Arguments::Write(args) => NfsRes::Write(backend.write(args, cred).await),
        Nfs3Arguments::Create(args) => NfsRes::Create(backend.create(args, cred).await),
        Nfs3Arguments::MkDir(args) => NfsRes::MkDir(backend.mk_dir(args, cred).await),
        Nfs3Arguments::SymLink(args) => NfsRes::SymLink(backend.symlink(args, cred).await),
        Nfs3Arguments::MkNod(args) => NfsRes::MkNod(backend.mk_node(args, cred).await),
        Nfs3Arguments::Remove(args) => NfsRes::Remove(backend.remove(args, cred).await),
        Nfs3Arguments::RmDir(args) => NfsRes::RmDir(backend.rm_dir(args, cred).await),
        Nfs3Arguments::Rename(args) => NfsRes::Rename(backend.rename(args, cred).await),
        Nfs3Arguments::Link(args) => NfsRes::Link(backend.link(args, cred).await),
        Nfs3Arguments::ReadDir(args) => NfsRes::ReadDir(backend.read_dir(args, cred).await),
        Nfs3Arguments::ReadDirPlus(args) => {
            NfsRes::ReadDirPlus(backend.read_dir_plus(args, cred).await)
        }
        Nfs3Arguments::FsStat(args) => NfsRes::FsStat(backend.fs_stat(args, cred).await),
        Nfs3Arguments::FsInfo(args) => NfsRes::FsInfo(backend.fs_info(args, cred).await),
        Nfs3Arguments::PathConf(args) => NfsRes::PathConf(backend.path_conf(args, cred).await),
        Nfs3Arguments::Commit(args) => NfsRes::Commit(backend.commit(args, cred).await),
    }
}
