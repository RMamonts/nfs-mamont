//! Service implementation for the MOUNT v3 `MNT` procedure.

use nfs_mamont::connection_id::ConnectionId;
use nfs_mamont::auth::Credential;
use nfs_mamont::mount::mnt::{Args, Fail, Mnt, Success};
use nfs_mamont::mount::{HostName, MountEntry};
use tracing::warn;

use super::MountService;
use super::AUTH;

impl Mnt for MountService {
    async fn mnt(
        &self,
        args: Args,
        connection_id: ConnectionId,
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
                client=%connection_id.into_inner(),
                configured_exports=?configured,
                "mount denied",
            );
            return Err(Fail::Access);
        };

        let file_handle = export.root_handle;

        let hostname = HostName::new(connection_id.into_inner().ip().to_string()).map_err(|_| Fail::Inval)?;

        let mount_entry = MountEntry { hostname, directory: args.dirpath.clone() };

        self.mounts.write().await.by_client.entry(connection_id).or_default().insert(mount_entry);

        Ok(Success { file_handle, auth_flavors: AUTH.to_vec() })
    }
}
