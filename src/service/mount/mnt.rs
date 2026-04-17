//! Service implementation for the MOUNT v3 `MNT` procedure.

use async_trait::async_trait;
use std::net::SocketAddr;

use tracing::warn;

use crate::mount::mnt::{Args, Fail, Mnt, Success};
use crate::mount::{HostName, MountEntry};
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
            warn!(
                requested=%args.dirpath.as_path().to_string_lossy(),
                client=%client_addr,
                configured_exports=?configured,
                "mount denied",
            );
            return Err(Fail::Access);
        };

        let file_handle = export.root_handle.clone();

        let mount_entry = MountEntry {
            hostname: HostName::new(client_addr.ip().to_string()).unwrap(),
            directory: args.dirpath.clone(),
        };

        self.mounts.by_client.entry(client_addr).or_default().insert(mount_entry);

        Ok(Success { file_handle, auth_flavors: AUTH.to_vec() })
    }
}
