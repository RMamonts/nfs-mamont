//! Service implementation for the MOUNT v3 `MNT` procedure.

use async_trait::async_trait;
use std::net::SocketAddr;

use crate::mount::mnt::{Args, Fail, Mnt, Success};
use crate::mount::MountEntry;
use crate::rpc::OpaqueAuth;

use super::MountService;
use super::AUTH;

#[async_trait]
impl Mnt for MountService {
    async fn mnt(
        &mut self,
        args: Args,
        client_addr: SocketAddr,
        _cred: OpaqueAuth,
    ) -> Result<Success, Fail> {
        let Some(export) = self.export_entry(&args.dirpath) else {
            let configured = self
                .exports
                .export_list()
                .into_iter()
                .map(|entry| entry.directory.as_path().to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            eprintln!(
                "mount denied: requested='{}' client={} configured_exports={configured:?}",
                args.dirpath.as_path().to_string_lossy(),
                client_addr,
            );
            return Err(Fail::Access);
        };

        let file_handle = export.root_handle.clone();

        let mount_entry =
            MountEntry { hostname: client_addr.ip().to_string(), directory: args.dirpath.clone() };

        self.mounts.by_client.entry(client_addr).or_default().insert(mount_entry);

        Ok(Success { file_handle, auth_flavors: AUTH.to_vec() })
    }
}
