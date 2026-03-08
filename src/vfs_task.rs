use std::io;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::mount;
use crate::parser::Arguments;
use crate::rpc::{CommandResult, RpcCommand, ServerContext, SharedVfs};
use crate::serializer::{serialize_reply, MountRes, NfsRes, ProcResult};

/// Process RPC commands, sends operation results to [`crate::write_task::WriteTask`].
pub struct VfsTask {
    command_receiver: UnboundedReceiver<RpcCommand>,
    result_sender: UnboundedSender<CommandResult>,
    server_context: ServerContext,
}

impl VfsTask {
    /// Creates new instance of [`VfsTask`].
    pub fn new(
        command_receiver: UnboundedReceiver<RpcCommand>,
        result_sender: UnboundedSender<CommandResult>,
        server_context: ServerContext,
    ) -> Self {
        Self { command_receiver, result_sender, server_context }
    }

    /// Spawns a [`VfsTask`].
    ///
    /// # Panics
    ///
    /// If called outside of tokio runtime context.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(mut self) {
        while let Some(command) = self.command_receiver.recv().await {
            let xid = command.context.header.xid;
            let result = serialize_reply(xid, self.dispatch(command).await).await;

            if self.result_sender.send(result).is_err() {
                break;
            }
        }
    }

    async fn dispatch(&self, command: RpcCommand) -> Result<ProcResult, crate::rpc::Error> {
        eprintln!(
            "rpc program={} version={} procedure={} auth={:?}",
            command.context.header.program,
            command.context.header.version,
            command.context.header.procedure,
            command.context.connection.auth(),
        );
        match *command.arguments {
            Arguments::Null => Ok(match command.context.header.program {
                crate::nfsv3::NFS_PROGRAM => ProcResult::Nfs3(NfsRes::Null),
                crate::mount::MOUNT_PROGRAM => ProcResult::Mount(MountRes::Null),
                _ => return Err(crate::rpc::Error::ProgramMismatch),
            }),
            other => self.dispatch_non_null(other, &command.context.connection).await,
        }
    }

    async fn dispatch_non_null(
        &self,
        arguments: Arguments,
        connection: &crate::rpc::ConnectionContext,
    ) -> Result<ProcResult, crate::rpc::Error> {
        match mount::MountRequest::try_from(arguments) {
            Ok(request) => {
                mount::MountService::new(connection, &self.server_context).dispatch(request).await
            }
            Err(other) => self.dispatch_nfs(other).await,
        }
    }

    async fn dispatch_nfs(&self, arguments: Arguments) -> Result<ProcResult, crate::rpc::Error> {
        macro_rules! dispatch_backend {
            ($args:ident, $method:ident, $variant:ident) => {
                Ok(ProcResult::Nfs3(NfsRes::$variant(self.backend()?.$method($args).await)))
            };
        }

        match arguments {
            Arguments::GetAttr(args) => dispatch_backend!(args, get_attr, GetAttr),
            Arguments::SetAttr(args) => dispatch_backend!(args, set_attr, SetAttr),
            Arguments::LookUp(args) => dispatch_backend!(args, lookup, LookUp),
            Arguments::Access(args) => dispatch_backend!(args, access, Access),
            Arguments::ReadLink(args) => dispatch_backend!(args, read_link, ReadLink),
            Arguments::Read(args) => dispatch_backend!(args, read, Read),
            Arguments::Write(args) => dispatch_backend!(args, write, Write),
            Arguments::Create(args) => dispatch_backend!(args, create, Create),
            Arguments::MkDir(args) => dispatch_backend!(args, mk_dir, MkDir),
            Arguments::SymLink(args) => dispatch_backend!(args, symlink, SymLink),
            Arguments::MkNod(args) => dispatch_backend!(args, mk_node, MkNod),
            Arguments::Remove(args) => dispatch_backend!(args, remove, Remove),
            Arguments::RmDir(args) => dispatch_backend!(args, rm_dir, RmDir),
            Arguments::Rename(args) => dispatch_backend!(args, rename, Rename),
            Arguments::Link(args) => dispatch_backend!(args, link, Link),
            Arguments::ReadDir(args) => dispatch_backend!(args, read_dir, ReadDir),
            Arguments::ReadDirPlus(args) => dispatch_backend!(args, read_dir_plus, ReadDirPlus),
            Arguments::FsStat(args) => dispatch_backend!(args, fs_stat, FsStat),
            Arguments::FsInfo(args) => dispatch_backend!(args, fs_info, FsInfo),
            Arguments::PathConf(args) => dispatch_backend!(args, path_conf, PathConf),
            Arguments::Commit(args) => dispatch_backend!(args, commit, Commit),
            _ => Err(crate::rpc::Error::ProcedureMismatch),
        }
    }

    fn backend(&self) -> Result<&SharedVfs, crate::rpc::Error> {
        self.server_context.backend().ok_or_else(|| {
            crate::rpc::Error::IO(io::Error::other("server backend is not configured"))
        })
    }
}
