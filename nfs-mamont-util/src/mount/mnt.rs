//! Service implementation for the MOUNT v3 `MNT` procedure.

use std::net::SocketAddr;

use nfs_mamont::auth::Credential;
use nfs_mamont::mount::mnt::{Args, Fail, Mnt, Success};
use nfs_mamont::mount::{HostName, MountEntry};
use tracing::warn;

use super::{ExportEntryWrapper, MountService};

impl Mnt for MountService {
    async fn mnt(
        &self,
        args: Args,
        client_addr: SocketAddr,
        cred: &Credential,
    ) -> Result<Success, Fail> {
        let Some(export) = self.export_entry(&args.dirpath).await else {
            let configured = self
                .exports
                .read()
                .await
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

        if let Err(denial) = export.policy.check(cred) {
            warn!(
                requested=%args.dirpath.as_path().to_string_lossy(),
                client=%client_addr,
                cred=?cred,
                reason=?denial,
                policy=?export.policy,
                "mount denied",
            );
            return Err(Fail::Access);
        }

        let ExportEntryWrapper { root_handle: file_handle, policy, .. } = export;

        let hostname = HostName::new(client_addr.ip().to_string()).map_err(|_| Fail::Inval)?;

        let mount_entry = MountEntry { hostname, directory: args.dirpath.clone() };

        self.mounts.write().await.by_client.entry(client_addr).or_default().insert(mount_entry);

        Ok(Success { file_handle, auth_flavors: policy.auth_flavors })
    }
}
