//! Service implementation for the MOUNT v3 `MNT` procedure.

use async_trait::async_trait;
use std::net::SocketAddr;

use crate::mount::mnt::{Args, Fail, Mnt, Success};
use crate::mount::MountEntry;
use crate::rpc::AuthFlavor;

use super::MountService;

#[async_trait]
impl Mnt for MountService {
    async fn mnt(&mut self, args: Args, client_addr: SocketAddr) -> Result<Success, Fail> {
        let is_exported = self.exports.by_directory.contains_key(&args.dirpath);

        if !is_exported {
            return Err(Fail::Access);
        }

        let Ok(file_handle) = self.vfs.path_to_handle(&args.dirpath).await else {
            return Err(Fail::NoEnt);
        };

        let mount_entry =
            MountEntry { hostname: client_addr.ip().to_string(), directory: args.dirpath.clone() };

        self.mounts.by_client.entry(client_addr).or_default().insert(mount_entry);

        // TODO: take auth_flavors from config
        Ok(Success { file_handle, auth_flavors: vec![AuthFlavor::None] })
    }
}
