use std::io;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::mount;
use crate::parser::Arguments;
use crate::rpc::{CommandResult, ConnectionContext, RpcCommand, ServerContext, SharedVfs};
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
            command.context.connection.auth,
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
        connection: &ConnectionContext,
    ) -> Result<ProcResult, crate::rpc::Error> {
        match arguments {
            Arguments::Mount(args) => {
                Ok(ProcResult::Mount(MountRes::Mount(self.mount(args, connection).await)))
            }
            Arguments::Dump => Ok(ProcResult::Mount(MountRes::Dump(self.dump_mounts().await))),
            Arguments::Unmount(args) => {
                self.unmount(args, connection).await;
                Ok(ProcResult::Mount(MountRes::Unmount))
            }
            Arguments::UnmountAll => {
                self.unmount_all(connection).await;
                Ok(ProcResult::Mount(MountRes::UnmountAll))
            }
            Arguments::Export => Ok(ProcResult::Mount(MountRes::Export(self.export_list().await))),
            other => self.dispatch_nfs(other).await,
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
        self.server_context.backend.as_ref().ok_or_else(|| {
            crate::rpc::Error::IO(io::Error::other("server backend is not configured"))
        })
    }

    async fn mount(
        &self,
        args: mount::mnt::MountArgs,
        connection: &ConnectionContext,
    ) -> mount::mnt::Result {
        let Some(export) = self.find_export(&args.0, connection).await else {
            return Err(mount::mnt::MntError::Access);
        };

        let backend = match self.backend() {
            Ok(backend) => backend,
            Err(_) => return Err(mount::mnt::MntError::ServerFault),
        };

        let file_handle = backend.root_handle().await;
        let client_addr = connection
            .client_addr
            .map(|addr| addr.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let mut mounts = self.server_context.mounts.write().await;
        mounts.push(crate::rpc::ServerMount { client_addr, directory: export.directory.clone() });

        Ok(mount::mnt::Success {
            file_handle,
            auth_flavors: vec![crate::rpc::AuthFlavor::Sys, crate::rpc::AuthFlavor::None],
        })
    }

    async fn find_export(
        &self,
        requested: &crate::vfs::file::Path,
        connection: &ConnectionContext,
    ) -> Option<crate::rpc::ServerExport> {
        let exports = self.server_context.exports.read().await;
        exports
            .iter()
            .find(|export| {
                export.directory.as_path() == requested.as_path()
                    && self.client_allowed(&export.allowed_hosts, connection)
            })
            .cloned()
    }

    fn client_allowed(&self, allowed_hosts: &[String], connection: &ConnectionContext) -> bool {
        if allowed_hosts.is_empty() {
            return true;
        }

        let Some(client_addr) = connection.client_addr else {
            return false;
        };
        let ip = client_addr.ip();
        let ip_text = ip.to_string();
        let socket_text = client_addr.to_string();

        allowed_hosts.iter().any(|allowed| {
            allowed == "*"
                || allowed == &ip_text
                || allowed == &socket_text
                || (allowed == "localhost" && ip.is_loopback())
        })
    }

    async fn dump_mounts(&self) -> mount::dump::Success {
        let mounts = self.server_context.mounts.read().await;
        let mount_list = mounts
            .iter()
            .cloned()
            .map(|mount| mount::MountEntry {
                hostname: mount.client_addr,
                directory: mount.directory,
            })
            .collect();

        mount::dump::Success { mount_list }
    }

    async fn export_list(&self) -> mount::export::Success {
        let exports = self.server_context.exports.read().await;
        let exports = exports
            .iter()
            .cloned()
            .map(|export| mount::ExportEntry {
                directory: export.directory,
                names: export.allowed_hosts,
            })
            .collect();

        mount::export::Success { exports }
    }

    async fn unmount(&self, args: mount::umnt::UnmountArgs, connection: &ConnectionContext) {
        let Some(client_addr) = connection.client_addr.map(|addr| addr.ip().to_string()) else {
            return;
        };

        let mut mounts = self.server_context.mounts.write().await;
        mounts.retain(|mount| {
            !(mount.client_addr == client_addr && mount.directory.as_path() == args.0.as_path())
        });
    }

    async fn unmount_all(&self, connection: &ConnectionContext) {
        let Some(client_addr) = connection.client_addr.map(|addr| addr.ip().to_string()) else {
            return;
        };

        let mut mounts = self.server_context.mounts.write().await;
        mounts.retain(|mount| mount.client_addr != client_addr);
    }
}
