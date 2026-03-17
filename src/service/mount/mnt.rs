//! Service implementation for the MOUNT v3 `MNT` procedure.

use async_trait::async_trait;
use std::net::SocketAddr;

use crate::mount::mnt::{Args, Fail, Mnt, Success};
use crate::mount::MountEntry;
use crate::rpc::{AuthFlavor, OpaqueAuth};

use super::MountService;

#[async_trait]
impl Mnt for MountService {
    async fn mnt(
        &mut self,
        args: Args,
        client_addr: SocketAddr,
        _cred: OpaqueAuth,
    ) -> Result<Success, Fail> {
        let Some(export) = self.export_entry(&args.dirpath) else {
            return Err(Fail::Access);
        };

        let file_handle = export.file_handle.clone();
        let auth_flavors = if export.auth_flavors.is_empty() {
            vec![AuthFlavor::None]
        } else {
            export.auth_flavors.clone()
        };

        let mount_entry =
            MountEntry { hostname: client_addr.ip().to_string(), directory: args.dirpath.clone() };

        self.mounts.by_client.entry(client_addr).or_default().insert(mount_entry);

        Ok(Success { file_handle, auth_flavors })
    }
}
