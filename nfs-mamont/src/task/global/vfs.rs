use async_channel::{Receiver, Sender};
use std::num::NonZeroUsize;
use std::sync::Arc;

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
/// Receiver from the queue, each [`VfsTask`] worker competes for the same command stream.
type VfsCommandReceiver<B> = Receiver<VfsCommand<B>>;

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

/// Pool of [`VfsTask`] workers fed from a single unbounded command channel.
///
/// Every worker is a separate tokio task that executes one procedure at a time, so:
/// - procedures run in parallel on all runtime worker threads: the CPU work done inside
///   backend futures (copying data, checksums, synchronous syscalls, ...) is spread over
///   the cores instead of being serialized in a single task;
/// - at most as many procedures as there are workers are executed against the backends
///   at the same time, the rest wait in the channel. The pool size is therefore the
///   backend queue depth.
pub struct VfsManager<B: Buffer> {
    /// Sender to enqueue work in the pool for execution.
    sender: VfsCommandSender<B>,
}

impl<B: Buffer + 'static> VfsManager<B> {
    /// Creates a new [`VfsManager`] with `concurrency` workers.
    ///
    /// # Parameters
    ///
    /// - `backends` --- registry of filesystem implementations, may be empty
    /// - `concurrency` --- number of workers, i.e. the maximum number of procedures
    ///   executed against the backends at the same time
    ///
    /// # Returns
    ///
    /// A new [`VfsManager`] whose workers are already running.
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn new<V>(backends: BackendRegistry<V>, concurrency: NonZeroUsize) -> Self
    where
        V: Vfs<B> + Send + Sync + 'static,
    {
        let (tx, rx) = async_channel::unbounded::<VfsCommand<B>>();

        (0..concurrency.get()).for_each(|_| {
            VfsTask::new(backends.clone(), rx.clone()).spawn();
        });

        Self { sender: tx }
    }

    /// Returns a clone of the command sender for enqueueing work in the queue.
    pub fn sender(&self) -> VfsCommandSender<B> {
        self.sender.clone()
    }
}

impl<B: Buffer> Drop for VfsManager<B> {
    /// Closes the pool's sender so workers stop after the channel is empty.
    fn drop(&mut self) {
        self.sender.close();
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
    /// Shared receiver from the pool, each worker competes for the same command stream.
    command_receiver: VfsCommandReceiver<B>,
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
    /// - `command_receiver` --- shared receiver from the pool
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

    /// Consumes commands until the channel is closed and empty, dispatching each NFS op
    /// and sending replies.
    async fn run(self) {
        while let Ok(command) = self.command_receiver.recv().await {
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

#[cfg(test)]
mod tests {
    use super::{failed_response, target, Target};
    use crate::allocator::Slice;
    use crate::parser::NfsArguments;
    use crate::vfs::file::{Handle, PAYLOAD_SIZE};
    use crate::vfs::{self, get_attr, link, rename, DirOpArgs, NfsRes};

    fn handle(backend: u8) -> Handle {
        Handle::new(backend, [0x01; PAYLOAD_SIZE])
    }

    fn name() -> vfs::file::Name {
        vfs::file::Name::new("file".to_string()).unwrap()
    }

    #[test]
    fn null_needs_no_backend() {
        let proc = NfsArguments::<Slice>::Null;

        assert!(matches!(target(&proc), Target::None));
    }

    #[test]
    fn backend_is_taken_from_the_handle() {
        let proc = NfsArguments::<Slice>::GetAttr(get_attr::Args { file: handle(3) });

        assert!(matches!(target(&proc), Target::Single(3)));
    }

    #[test]
    fn rename_within_one_backend_is_routed_to_it() {
        let proc = NfsArguments::<Slice>::Rename(rename::Args {
            from: DirOpArgs { dir: handle(2), name: name() },
            to: DirOpArgs { dir: handle(2), name: name() },
        });

        assert!(matches!(target(&proc), Target::Single(2)));
    }

    #[test]
    fn rename_across_backends_is_crossing() {
        let proc = NfsArguments::<Slice>::Rename(rename::Args {
            from: DirOpArgs { dir: handle(2), name: name() },
            to: DirOpArgs { dir: handle(5), name: name() },
        });

        assert!(matches!(target(&proc), Target::Crossing));
    }

    #[test]
    fn link_across_backends_is_crossing() {
        let proc = NfsArguments::<Slice>::Link(link::Args {
            file: handle(0),
            link: DirOpArgs { dir: handle(1), name: name() },
        });

        assert!(matches!(target(&proc), Target::Crossing));
    }

    #[test]
    fn failed_response_keeps_the_procedure_variant() {
        let proc = NfsArguments::<Slice>::GetAttr(get_attr::Args { file: handle(0) });

        let NfsRes::GetAttr(Err(fail)) = failed_response(&proc, vfs::Error::StaleFile) else {
            panic!("expected a failed GETATTR response");
        };
        assert_eq!(fail.error, vfs::Error::StaleFile);
    }

    /// Scheduling of procedures by [`super::VfsManager`], observed through a [`Probe`] backend.
    mod pool {
        use std::num::NonZeroUsize;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Condvar, Mutex};
        use std::time::Duration;

        use async_channel::Sender;
        use tokio::sync::Semaphore;

        use super::super::{VfsCommand, VfsManager};
        use super::handle;
        use crate::allocator::{Buffer, Slice};
        use crate::backend::BackendRegistry;
        use crate::parser::{NfsArgWrapper, NfsArguments, RpcHeader};
        use crate::rpc::auth::Credential;
        use crate::task::{ProcReply, ProcResult};
        use crate::vfs::{self, file, get_attr, NfsRes};

        /// Backend that serves only GETATTR and records how its calls are scheduled.
        struct Probe {
            /// Number of GETATTR calls being executed right now.
            in_flight: AtomicUsize,
            /// Highest value [`Probe::in_flight`] has reached.
            max_in_flight: AtomicUsize,
            /// Number of calls that must block their threads at the same time before any
            /// of them may continue; `0` disables the rendezvous.
            parties: usize,
            /// Number of calls that reached the rendezvous.
            arrived: Mutex<usize>,
            /// Signalled whenever [`Probe::arrived`] changes.
            arrived_changed: Condvar,
            /// Every call takes a permit from the gate before it completes.
            gate: Semaphore,
        }

        impl Probe {
            fn new(parties: usize, gate_permits: usize) -> Self {
                Self {
                    in_flight: AtomicUsize::new(0),
                    max_in_flight: AtomicUsize::new(0),
                    parties,
                    arrived: Mutex::new(0),
                    arrived_changed: Condvar::new(),
                    gate: Semaphore::new(gate_permits),
                }
            }

            /// Blocks the calling thread until [`Probe::parties`] calls are blocked here.
            ///
            /// Returns `false` if that has not happened within a few seconds.
            fn rendezvous(&self) -> bool {
                if self.parties == 0 {
                    return true;
                }

                let mut arrived = self.arrived.lock().unwrap();
                *arrived += 1;
                self.arrived_changed.notify_all();

                let (_arrived, wait) = self
                    .arrived_changed
                    .wait_timeout_while(arrived, Duration::from_secs(5), |arrived| {
                        *arrived < self.parties
                    })
                    .unwrap();

                !wait.timed_out()
            }
        }

        impl get_attr::GetAttr for Probe {
            async fn get_attr(
                &self,
                _args: get_attr::Args,
                _cred: &Credential,
            ) -> Result<get_attr::Success, get_attr::Fail> {
                let in_flight = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_in_flight.fetch_max(in_flight, Ordering::SeqCst);

                let met = self.rendezvous();
                self.gate.acquire().await.expect("the gate is never closed").forget();

                self.in_flight.fetch_sub(1, Ordering::SeqCst);

                if met {
                    Ok(get_attr::Success { object: attr() })
                } else {
                    Err(get_attr::Fail { error: vfs::Error::JUKEBOX })
                }
            }
        }

        /// Implements the procedures that the tests never send.
        macro_rules! unused_procedures {
            ($($module:ident::$procedure:ident::$method:ident),* $(,)?) => {
                $(
                    impl vfs::$module::$procedure for Probe {
                        async fn $method(
                            &self,
                            _args: vfs::$module::Args,
                            _cred: &Credential,
                        ) -> Result<vfs::$module::Success, vfs::$module::Fail> {
                            unreachable!("the tests send only GETATTR")
                        }
                    }
                )*
            };
        }

        unused_procedures!(
            set_attr::SetAttr::set_attr,
            lookup::Lookup::lookup,
            access::Access::access,
            read_link::ReadLink::read_link,
            create::Create::create,
            mk_dir::MkDir::mk_dir,
            symlink::Symlink::symlink,
            mk_node::MkNode::mk_node,
            remove::Remove::remove,
            rm_dir::RmDir::rm_dir,
            rename::Rename::rename,
            link::Link::link,
            read_dir::ReadDir::read_dir,
            read_dir_plus::ReadDirPlus::read_dir_plus,
            fs_stat::FsStat::fs_stat,
            fs_info::FsInfo::fs_info,
            path_conf::PathConf::path_conf,
            commit::Commit::commit,
        );

        impl<B: Buffer> vfs::read::Read<B> for Probe {
            async fn read(
                &self,
                _args: vfs::read::Args,
                _data: B,
                _cred: &Credential,
            ) -> Result<vfs::read::Success<B>, vfs::read::Fail> {
                unreachable!("the tests send only GETATTR")
            }
        }

        impl<B: Buffer> vfs::write::Write<B> for Probe {
            async fn write(
                &self,
                _args: vfs::write::Args<B>,
                _cred: &Credential,
            ) -> Result<vfs::write::Success, vfs::write::Fail> {
                unreachable!("the tests send only GETATTR")
            }
        }

        fn attr() -> file::Attr {
            let time = file::Time { seconds: 0, nanos: 0 };

            file::Attr {
                file_type: file::Type::Regular,
                mode: 0o644,
                nlink: 1,
                uid: 0,
                gid: 0,
                size: 0,
                used: 0,
                device: file::Device { major: 0, minor: 0 },
                fs_id: 0,
                file_id: 1,
                atime: time,
                mtime: time,
                ctime: time,
            }
        }

        /// Starts a pool of `concurrency` workers serving `probe` as backend `0`.
        fn start(probe: &Arc<Probe>, concurrency: usize) -> VfsManager<Slice> {
            let backends = BackendRegistry::new();
            assert_eq!(backends.add(Arc::clone(probe)), Some(0));

            VfsManager::new(backends, NonZeroUsize::new(concurrency).unwrap())
        }

        fn get_attr_command(xid: u32, reply: &Sender<ProcReply<Slice>>) -> VfsCommand<Slice> {
            let proc = NfsArguments::GetAttr(get_attr::Args { file: handle(0) });
            let header = RpcHeader { xid, cred: Credential::None };

            (NfsArgWrapper { header, proc: Box::new(proc) }, reply.clone())
        }

        fn succeeded(reply: ProcReply<Slice>) -> bool {
            match reply.proc_result {
                Ok(ProcResult::Nfs3(response)) => matches!(*response, NfsRes::GetAttr(Ok(_))),
                _ => false,
            }
        }

        /// Every call blocks its thread until all calls are blocked at once, which is
        /// possible only if procedures are executed in parallel rather than just
        /// concurrently within a single task.
        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn procedures_run_in_parallel() {
            const PARALLEL: usize = 4;

            let probe = Arc::new(Probe::new(PARALLEL, Semaphore::MAX_PERMITS));
            let manager = start(&probe, PARALLEL);
            let (reply_tx, reply_rx) = async_channel::unbounded();

            for xid in 0..PARALLEL as u32 {
                manager.sender().send(get_attr_command(xid, &reply_tx)).await.unwrap();
            }

            for _ in 0..PARALLEL {
                let reply = reply_rx.recv().await.unwrap();
                assert!(succeeded(reply), "procedures were not executed in parallel");
            }
        }

        #[tokio::test]
        async fn in_flight_procedures_are_bounded() {
            const CONCURRENCY: usize = 2;
            const COMMANDS: usize = 8;

            let probe = Arc::new(Probe::new(0, 0));
            let manager = start(&probe, CONCURRENCY);
            let (reply_tx, reply_rx) = async_channel::unbounded();

            for xid in 0..COMMANDS as u32 {
                manager.sender().send(get_attr_command(xid, &reply_tx)).await.unwrap();
            }

            // Give the workers every chance to start more procedures than allowed.
            for _ in 0..COMMANDS * 4 {
                tokio::task::yield_now().await;
            }
            assert_eq!(probe.in_flight.load(Ordering::SeqCst), CONCURRENCY);

            probe.gate.add_permits(COMMANDS);
            for _ in 0..COMMANDS {
                assert!(succeeded(reply_rx.recv().await.unwrap()));
            }
            assert_eq!(probe.max_in_flight.load(Ordering::SeqCst), CONCURRENCY);
        }
    }
}
