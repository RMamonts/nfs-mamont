use std::io;

use super::{dump, export, mnt, umnt};
use crate::parser::Arguments;
use crate::rpc::{ConnectionContext, ServerContext, SharedVfs};
use crate::serializer::{MountRes, ProcResult};

pub(crate) struct MountService<'a> {
    connection: &'a ConnectionContext,
    server_context: &'a ServerContext,
}

impl<'a> MountService<'a> {
    pub(crate) fn new(
        connection: &'a ConnectionContext,
        server_context: &'a ServerContext,
    ) -> Self {
        Self { connection, server_context }
    }

    pub(crate) async fn dispatch(
        &self,
        request: MountRequest,
    ) -> Result<ProcResult, crate::rpc::Error> {
        let result = match request {
            MountRequest::Mount(args) => MountRes::Mount(self.mount(args).await),
            MountRequest::Dump => MountRes::Dump(self.dump_mounts().await),
            MountRequest::Unmount(args) => {
                self.unmount(args).await;
                MountRes::Unmount
            }
            MountRequest::UnmountAll => {
                self.unmount_all().await;
                MountRes::UnmountAll
            }
            MountRequest::Export => MountRes::Export(self.export_list().await),
        };

        Ok(ProcResult::Mount(result))
    }

    fn backend(&self) -> Result<&SharedVfs, crate::rpc::Error> {
        self.server_context.backend.as_ref().ok_or_else(|| {
            crate::rpc::Error::IO(io::Error::other("server backend is not configured"))
        })
    }

    async fn mount(&self, args: mnt::MountArgs) -> mnt::Result {
        let Some(export) = self.find_export(&args.0).await else {
            return Err(mnt::MntError::Access);
        };

        let backend = match self.backend() {
            Ok(backend) => backend,
            Err(_) => return Err(mnt::MntError::ServerFault),
        };

        let file_handle = backend.root_handle().await;
        let client_addr = self
            .connection
            .client_addr
            .map(|addr| addr.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let mut mounts = self.server_context.mounts.write().await;
        mounts.push(crate::rpc::ServerMount { client_addr, directory: export.directory.clone() });

        Ok(mnt::Success {
            file_handle,
            auth_flavors: vec![crate::rpc::AuthFlavor::Sys, crate::rpc::AuthFlavor::None],
        })
    }

    async fn find_export(
        &self,
        requested: &crate::vfs::file::Path,
    ) -> Option<crate::rpc::ServerExport> {
        let exports = self.server_context.exports.read().await;
        exports
            .iter()
            .find(|export| {
                export.directory.as_path() == requested.as_path()
                    && self.client_allowed(&export.allowed_hosts)
            })
            .cloned()
    }

    fn client_allowed(&self, allowed_hosts: &[String]) -> bool {
        if allowed_hosts.is_empty() {
            return true;
        }

        let Some(client_addr) = self.connection.client_addr else {
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

    async fn dump_mounts(&self) -> dump::Success {
        let mounts = self.server_context.mounts.read().await;
        let mount_list = mounts
            .iter()
            .cloned()
            .map(|mount| crate::mount::MountEntry {
                hostname: mount.client_addr,
                directory: mount.directory,
            })
            .collect();

        dump::Success { mount_list }
    }

    async fn export_list(&self) -> export::Success {
        let exports = self.server_context.exports.read().await;
        let exports = exports
            .iter()
            .cloned()
            .map(|export| crate::mount::ExportEntry {
                directory: export.directory,
                names: export.allowed_hosts,
            })
            .collect();

        export::Success { exports }
    }

    async fn unmount(&self, args: umnt::UnmountArgs) {
        let Some(client_addr) = self.connection.client_addr.map(|addr| addr.ip().to_string())
        else {
            return;
        };

        let mut mounts = self.server_context.mounts.write().await;
        mounts.retain(|mount| {
            !(mount.client_addr == client_addr && mount.directory.as_path() == args.0.as_path())
        });
    }

    async fn unmount_all(&self) {
        let Some(client_addr) = self.connection.client_addr.map(|addr| addr.ip().to_string())
        else {
            return;
        };

        let mut mounts = self.server_context.mounts.write().await;
        mounts.retain(|mount| mount.client_addr != client_addr);
    }
}

pub(crate) enum MountRequest {
    Mount(mnt::MountArgs),
    Dump,
    Unmount(umnt::UnmountArgs),
    UnmountAll,
    Export,
}

impl TryFrom<Arguments> for MountRequest {
    type Error = Arguments;

    fn try_from(arguments: Arguments) -> Result<Self, Self::Error> {
        match arguments {
            Arguments::Mount(args) => Ok(Self::Mount(args)),
            Arguments::Dump => Ok(Self::Dump),
            Arguments::Unmount(args) => Ok(Self::Unmount(args)),
            Arguments::UnmountAll => Ok(Self::UnmountAll),
            Arguments::Export => Ok(Self::Export),
            other => Err(other),
        }
    }
}
