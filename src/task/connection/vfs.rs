use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::{Mutex, Semaphore};
use tracing::error;

use crate::allocator::{Allocator, Impl, Slice};
use crate::context::ServerContext;
use crate::parser::{NfsArgWrapper, NfsArguments};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::{self, NfsRes, Vfs};

/// Process RPC commands, sends operation results to [`crate::task::connection::write::WriteTask`].
pub struct VfsTask {
    backend: Arc<dyn Vfs + Send + Sync + 'static>,
    allocator: Arc<Mutex<Impl>>,
    command_receiver: mpsc::Receiver<NfsArgWrapper>,
    result_sender: mpsc::Sender<ProcReply>,
}

const MAX_IN_FLIGHT_REQUESTS_PER_CONNECTION: usize = 16;

impl VfsTask {
    /// Creates new instance of [`VfsTask`].
    pub fn new(
        context: &ServerContext,
        command_receiver: mpsc::Receiver<NfsArgWrapper>,
        result_sender: mpsc::Sender<ProcReply>,
    ) -> Self {
        Self {
            backend: context.get_backend(),
            allocator: context.get_read_allocator(),
            command_receiver,
            result_sender,
        }
    }

    /// Spawns a [`VfsTask`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let mut command_receiver = self.command_receiver;
        let limiter = Arc::new(Semaphore::new(MAX_IN_FLIGHT_REQUESTS_PER_CONNECTION));

        while let Some(command) = command_receiver.recv().await {
            let permit = match limiter.clone().acquire_owned().await {
                Ok(permit) => permit,
                Err(_) => return,
            };

            let backend = Arc::clone(&self.backend);
            let allocator = Arc::clone(&self.allocator);
            let result_sender = self.result_sender.clone();

            tokio::spawn(async move {
                let _permit = permit;
                let reply = Self::process_command(backend, allocator, command).await;
                if result_sender.send(reply).await.is_err() {
                    // Write task may already be closed; then this connection pipeline is done.
                }
            });
        }
    }

    async fn process_command(
        backend: Arc<dyn Vfs + Send + Sync + 'static>,
        allocator: Arc<Mutex<Impl>>,
        command: NfsArgWrapper,
    ) -> ProcReply {
        let NfsArgWrapper { header, proc } = command;
        let proc_name = Self::proc_name(&proc);

        let response = match *proc {
            NfsArguments::Null => NfsRes::Null,
            NfsArguments::GetAttr(args) => NfsRes::GetAttr(backend.get_attr(args).await),
            NfsArguments::SetAttr(args) => NfsRes::SetAttr(backend.set_attr(args).await),
            NfsArguments::LookUp(args) => NfsRes::LookUp(backend.lookup(args).await),
            NfsArguments::Access(args) => NfsRes::Access(backend.access(args).await),
            NfsArguments::ReadLink(args) => NfsRes::ReadLink(backend.read_link(args).await),
            NfsArguments::Read(args) => {
                let data_result = if args.count == 0 {
                    Ok(Slice::empty())
                } else {
                    let requested_size = NonZeroUsize::new(args.count as usize).unwrap();

                    let mut allocator = allocator.lock().await;
                    allocator
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
        };

        if let Some(error) = Self::error_from_response(&response) {
            error!(xid=header.xid, proc=%proc_name, error=?error, "nfs op failed");
        }

        ProcReply { xid: header.xid, proc_result: Ok(ProcResult::Nfs3(Box::new(response))) }
    }

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
