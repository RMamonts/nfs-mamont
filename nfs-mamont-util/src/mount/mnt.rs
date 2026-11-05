//! Service implementation for the MOUNT v3 `MNT` procedure.

use nfs_mamont::auth::Credential;
use nfs_mamont::client_id::ConnectionInfo;
use nfs_mamont::mount::mnt::{Args, Fail, Mnt, Success};
use nfs_mamont::mount::{HostName, MountEntry};
use tracing::warn;

use super::MountService;
use super::AUTH;

impl Mnt for MountService {
    async fn mnt(
        &self,
        args: Args,
        connection: ConnectionInfo,
        _cred: &Credential,
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
                client=%connection.peer_addr,
                configured_exports=?configured,
                "mount denied",
            );
            return Err(Fail::Access);
        };

        let file_handle = export.root_handle;

        let ip = connection.peer_addr.ip().to_canonical();
        let hostname = HostName::new(ip.to_string()).map_err(|_| Fail::Inval)?;

        let mount_entry = MountEntry { hostname, directory: args.dirpath.clone() };

        self.mounts.write().await.by_client.entry(ip).or_default().insert(mount_entry);

        Ok(Success { file_handle, auth_flavors: AUTH.to_vec() })
    }
}
