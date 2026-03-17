use crate::parser::NfsArgWrapper;
use crate::parser::NfsArguments;
use crate::task::ProcReply;
use crate::task::ProcResult;
use crate::vfs::{NfsRes, Vfs};
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// Process RPC commands, sends operation results to [`crate::task::connection::write::WriteTask`].
pub struct VfsTask {
    backend: Arc<dyn Vfs + Send + Sync + 'static>,
    command_receiver: UnboundedReceiver<NfsArgWrapper>,
    result_sender: UnboundedSender<ProcReply>,
}

impl VfsTask {
    /// Creates new instance of [`VfsTask`].
    pub fn new(
        backend: Arc<dyn Vfs + Send + Sync + 'static>,
        command_receiver: UnboundedReceiver<NfsArgWrapper>,
        result_sender: UnboundedSender<ProcReply>,
    ) -> Self {
        Self { backend, command_receiver, result_sender }
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

        while let Some(command) = command_receiver.recv().await {
            let NfsArgWrapper { header, proc } = command;

            let response = match *proc {
                NfsArguments::Null => {
                    unreachable!()
                }
                NfsArguments::GetAttr(args) => NfsRes::GetAttr(self.backend.get_attr(args).await),
                NfsArguments::SetAttr(args) => NfsRes::SetAttr(self.backend.set_attr(args).await),
                NfsArguments::LookUp(args) => NfsRes::LookUp(self.backend.lookup(args).await),
                NfsArguments::Access(args) => NfsRes::Access(self.backend.access(args).await),
                NfsArguments::ReadLink(args) => {
                    NfsRes::ReadLink(self.backend.read_link(args).await)
                }
                NfsArguments::Read(args) => NfsRes::Read(self.backend.read(args).await),
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

            let reply = ProcReply {
                xid: header.xid,
                proc_result: Ok(ProcResult::Nfs3(Box::new(response))),
            };

            // Write task may already be closed; then this connection pipeline is done.
            if self.result_sender.send(reply).is_err() {
                return;
            }
        }
    }
}
