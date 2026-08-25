use async_channel::{Receiver, Sender};
use std::num::NonZeroUsize;
use std::sync::Arc;

use tracing::{error, warn};

use crate::allocator::{Allocator, Buffer};
use crate::backend::BackendRegistry;
use crate::parser::{NfsArgWrapper, NfsArguments};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::file::BackendId;
use crate::vfs::{self, NfsRes, Vfs};

/// One queued NFS procedure: parsed arguments and a channel to send the result.
pub type VfsCommand<BR, BW> = (NfsArgWrapper<BW>, Sender<ProcReply<BR>>);
/// Sender to enqueue work in the pool.
pub type VfsCommandSender<BR, BW> = Sender<VfsCommand<BR, BW>>;
/// Receiver from the pool, each worker competes for the same command stream.
type VfsCommandReceiver<BR, BW> = Receiver<VfsCommand<BR, BW>>;

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
        NfsArguments::Read(args) => (&args.file, None),
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
fn failed_response<BR: Buffer, BW: Buffer>(
    proc: &NfsArguments<BW>,
    error: vfs::Error,
) -> NfsRes<BR> {
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
        NfsArguments::Read(_) => NfsRes::Read(Err(vfs::read::Fail { error, file_attr: None })),
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

/// Fixed-size pool of [`VfsTask`] workers fed from a single unbounded command channel.
pub struct VfsPool<BR: Buffer, BW: Buffer> {
    /// Sender to enqueue work in the pool for execution.
    sender: VfsCommandSender<BR, BW>,
}

impl<BR: Buffer + 'static, BW: Buffer + 'static> VfsPool<BR, BW> {
    /// Creates a new [`VfsPool`] with the given number of workers.
    ///
    /// # Parameters
    ///
    /// - `num` --- number of workers to create
    /// - `backends` --- registry of filesystem implementations, may be empty
    /// - `allocator` --- allocator used for read buffers
    ///
    /// # Returns
    ///
    /// A new [`VfsPool`] with the given number of workers.
    pub fn new<A, V>(num: NonZeroUsize, backends: BackendRegistry<V>, allocator: Arc<A>) -> Self
    where
        A: Allocator<Buffer = BR> + Send + Sync + 'static,
        V: Vfs<BR, BW> + Send + Sync + 'static,
    {
        let (tx, rx) = async_channel::unbounded::<VfsCommand<BR, BW>>();

        (0..num.get()).for_each(|_| {
            let rx_clone = rx.clone();
            VfsTask::new(backends.clone(), Arc::clone(&allocator), rx_clone).spawn();
        });

        Self { sender: tx }
    }

    /// Returns a clone of the command sender for enqueueing work in the pool.
    pub fn sender(&self) -> VfsCommandSender<BR, BW> {
        self.sender.clone()
    }
}

impl<BR: Buffer, BW: Buffer> Drop for VfsPool<BR, BW> {
    /// Closes the pool's sender so workers stop after channel is empty.
    fn drop(&mut self) {
        self.sender.close();
    }
}

/// Task that executes NFS procedures against [`Vfs`] and sends the result to the writer pipeline.
pub struct VfsTask<A, V, BR, BW>
where
    A: Allocator<Buffer = BR> + Send + Sync + 'static,
    BR: Buffer,
    BW: Buffer,
    V: vfs::Vfs<BR, BW> + Send + Sync + 'static,
{
    /// Registry of filesystem implementations, indexed by the first byte of a file handle.
    backends: BackendRegistry<V>,
    /// Allocator used for read buffers.
    allocator: Arc<A>,
    /// Shared receiver from the pool, each worker competes for the same command stream.
    command_receiver: VfsCommandReceiver<BR, BW>,
}

impl<A, V, BR, BW> VfsTask<A, V, BR, BW>
where
    A: Allocator<Buffer = BR> + Send + Sync + 'static,
    BR: Buffer + 'static,
    BW: Buffer + 'static,
    V: vfs::Vfs<BR, BW> + Send + Sync + 'static,
{
    /// Builds a worker that reads commands from the pool and executes them.
    ///
    /// # Parameters
    ///
    /// - `backends` --- registry of filesystem implementations, may be empty
    /// - `allocator` --- allocator used for read buffers
    /// - `command_receiver` --- receiver from the pool
    ///
    /// # Returns
    ///
    /// A new [`VfsTask`] that reads commands from the pool and executes them.
    pub fn new(
        backends: BackendRegistry<V>,
        allocator: Arc<A>,
        command_receiver: VfsCommandReceiver<BR, BW>,
    ) -> Self {
        Self { backends, allocator, command_receiver }
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
        let command_receiver = self.command_receiver.clone();

        while let Ok((command, tx)) = command_receiver.recv().await {
            let NfsArgWrapper { header, proc } = command;
            let proc_name = Self::proc_name(&proc);

            let response = match target(&proc) {
                Target::None => NfsRes::Null,
                Target::Crossing => failed_response(&proc, vfs::Error::XDev),
                Target::Single(backend_id) => match self.backends.get(backend_id) {
                    Some(backend) => self.execute(*proc, backend).await,
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

    /// Runs the procedure against `backend` and wraps its result.
    async fn execute(&self, proc: NfsArguments<BW>, backend: Arc<V>) -> NfsRes<BR> {
        match proc {
            NfsArguments::Null => NfsRes::Null,
            NfsArguments::GetAttr(args) => NfsRes::GetAttr(backend.get_attr(args).await),
            NfsArguments::SetAttr(args) => NfsRes::SetAttr(backend.set_attr(args).await),
            NfsArguments::LookUp(args) => NfsRes::LookUp(backend.lookup(args).await),
            NfsArguments::Access(args) => NfsRes::Access(backend.access(args).await),
            NfsArguments::ReadLink(args) => NfsRes::ReadLink(backend.read_link(args).await),
            NfsArguments::Read(args) => {
                let data_result = if args.count == 0 {
                    Ok(BR::empty())
                } else {
                    let requested_size = NonZeroUsize::new(args.count as usize).unwrap();

                    self.allocator
                        .allocate(requested_size)
                        .await
                        .ok_or(vfs::read::Fail { error: vfs::Error::TooSmall, file_attr: None })
                };

                match data_result {
                    Ok(data) => NfsRes::Read(backend.read(args, data).await),
                    Err(err) => NfsRes::Read(Err(err)),
                }
            }
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
            NfsArguments::ReadDirPlus(args) => {
                NfsRes::ReadDirPlus(backend.read_dir_plus(args).await)
            }
            NfsArguments::FsStat(args) => NfsRes::FsStat(backend.fs_stat(args).await),
            NfsArguments::FsInfo(args) => NfsRes::FsInfo(backend.fs_info(args).await),
            NfsArguments::PathConf(args) => NfsRes::PathConf(backend.path_conf(args).await),
            NfsArguments::Commit(args) => NfsRes::Commit(backend.commit(args).await),
        }
    }

    /// Static label for logging/tracing for the given procedure variant.
    fn proc_name(proc: &NfsArguments<BW>) -> &'static str {
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
    fn error_from_response(response: &NfsRes<BR>) -> Option<vfs::Error> {
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

        let NfsRes::GetAttr(Err(fail)): NfsRes<Slice> =
            failed_response(&proc, vfs::Error::StaleFile)
        else {
            panic!("expected a failed GETATTR response");
        };
        assert_eq!(fail.error, vfs::Error::StaleFile);
    }
}
