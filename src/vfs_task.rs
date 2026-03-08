use std::io;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::mount;
use crate::parser::Arguments;
use crate::rpc::{
    CommandResult, ConnectionContext, RpcCommand, RpcReply, ServerContext, SharedVfs,
};
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
            let result = match serialize_reply(xid, self.dispatch(command).await).await {
                Ok(payload) => Ok(RpcReply::new(xid, payload)),
                Err(err) => Err(err),
            };

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
            Arguments::GetAttr(args) => {
                Ok(ProcResult::Nfs3(NfsRes::GetAttr(self.backend()?.get_attr(args).await)))
            }
            Arguments::SetAttr(args) => {
                Ok(ProcResult::Nfs3(NfsRes::SetAttr(self.backend()?.set_attr(args).await)))
            }
            Arguments::LookUp(args) => {
                Ok(ProcResult::Nfs3(NfsRes::LookUp(self.backend()?.lookup(args).await)))
            }
            Arguments::Access(args) => {
                Ok(ProcResult::Nfs3(NfsRes::Access(self.backend()?.access(args).await)))
            }
            Arguments::ReadLink(args) => {
                Ok(ProcResult::Nfs3(NfsRes::ReadLink(self.backend()?.read_link(args).await)))
            }
            Arguments::Read(args) => {
                Ok(ProcResult::Nfs3(NfsRes::Read(self.backend()?.read(args).await)))
            }
            Arguments::Write(args) => {
                Ok(ProcResult::Nfs3(NfsRes::Write(self.backend()?.write(args).await)))
            }
            Arguments::Create(args) => {
                Ok(ProcResult::Nfs3(NfsRes::Create(self.backend()?.create(args).await)))
            }
            Arguments::MkDir(args) => {
                Ok(ProcResult::Nfs3(NfsRes::MkDir(self.backend()?.mk_dir(args).await)))
            }
            Arguments::SymLink(args) => {
                Ok(ProcResult::Nfs3(NfsRes::SymLink(self.backend()?.symlink(args).await)))
            }
            Arguments::MkNod(args) => {
                Ok(ProcResult::Nfs3(NfsRes::MkNod(self.backend()?.mk_node(args).await)))
            }
            Arguments::Remove(args) => {
                Ok(ProcResult::Nfs3(NfsRes::Remove(self.backend()?.remove(args).await)))
            }
            Arguments::RmDir(args) => {
                Ok(ProcResult::Nfs3(NfsRes::RmDir(self.backend()?.rm_dir(args).await)))
            }
            Arguments::Rename(args) => {
                Ok(ProcResult::Nfs3(NfsRes::Rename(self.backend()?.rename(args).await)))
            }
            Arguments::Link(args) => {
                Ok(ProcResult::Nfs3(NfsRes::Link(self.backend()?.link(args).await)))
            }
            Arguments::ReadDir(args) => {
                Ok(ProcResult::Nfs3(NfsRes::ReadDir(self.backend()?.read_dir(args).await)))
            }
            Arguments::ReadDirPlus(args) => {
                Ok(ProcResult::Nfs3(NfsRes::ReadDirPlus(self.backend()?.read_dir_plus(args).await)))
            }
            Arguments::FsStat(args) => {
                Ok(ProcResult::Nfs3(NfsRes::FsStat(self.backend()?.fs_stat(args).await)))
            }
            Arguments::FsInfo(args) => {
                Ok(ProcResult::Nfs3(NfsRes::FsInfo(self.backend()?.fs_info(args).await)))
            }
            Arguments::PathConf(args) => {
                Ok(ProcResult::Nfs3(NfsRes::PathConf(self.backend()?.path_conf(args).await)))
            }
            Arguments::Commit(args) => {
                Ok(ProcResult::Nfs3(NfsRes::Commit(self.backend()?.commit(args).await)))
            }
            Arguments::Mount(args) => Ok(ProcResult::Mount(MountRes::Mount(
                self.mount(args, &command.context.connection).await,
            ))),
            Arguments::Dump => Ok(ProcResult::Mount(MountRes::Dump(self.dump_mounts()))),
            Arguments::Unmount(args) => {
                self.unmount(args, &command.context.connection);
                Ok(ProcResult::Mount(MountRes::Unmount))
            }
            Arguments::UnmountAll => {
                self.unmount_all(&command.context.connection);
                Ok(ProcResult::Mount(MountRes::UnmountAll))
            }
            Arguments::Export => Ok(ProcResult::Mount(MountRes::Export(self.export_list()))),
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
        let Some(export) = self.find_export(&args.0, connection) else {
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

        if let Ok(mut mounts) = self.server_context.mounts.write() {
            mounts
                .push(crate::rpc::ServerMount { client_addr, directory: export.directory.clone() });
        }

        Ok(mount::mnt::Success {
            file_handle,
            auth_flavors: vec![crate::rpc::AuthFlavor::Sys, crate::rpc::AuthFlavor::None],
        })
    }

    fn find_export(
        &self,
        requested: &crate::vfs::file::Path,
        connection: &ConnectionContext,
    ) -> Option<crate::rpc::ServerExport> {
        let exports = self.server_context.exports.read().ok()?;
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

    fn dump_mounts(&self) -> mount::dump::Success {
        let mount_list = self
            .server_context
            .mounts
            .read()
            .map(|mounts| {
                mounts
                    .iter()
                    .cloned()
                    .map(|mount| mount::MountEntry {
                        hostname: mount.client_addr,
                        directory: mount.directory,
                    })
                    .collect()
            })
            .unwrap_or_default();

        mount::dump::Success { mount_list }
    }

    fn export_list(&self) -> mount::export::Success {
        let exports = self
            .server_context
            .exports
            .read()
            .map(|exports| {
                exports
                    .iter()
                    .cloned()
                    .map(|export| mount::ExportEntry {
                        directory: export.directory,
                        names: export.allowed_hosts,
                    })
                    .collect()
            })
            .unwrap_or_default();

        mount::export::Success { exports }
    }

    fn unmount(&self, args: mount::umnt::UnmountArgs, connection: &ConnectionContext) {
        let Some(client_addr) = connection.client_addr.map(|addr| addr.ip().to_string()) else {
            return;
        };

        if let Ok(mut mounts) = self.server_context.mounts.write() {
            mounts.retain(|mount| {
                !(mount.client_addr == client_addr && mount.directory.as_path() == args.0.as_path())
            });
        }
    }

    fn unmount_all(&self, connection: &ConnectionContext) {
        let Some(client_addr) = connection.client_addr.map(|addr| addr.ip().to_string()) else {
            return;
        };

        if let Ok(mut mounts) = self.server_context.mounts.write() {
            mounts.retain(|mount| mount.client_addr != client_addr);
        }
    }
}
