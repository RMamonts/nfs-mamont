//! Service implementation for the MOUNT v3 `MNT` procedure.

use async_trait::async_trait;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;

use crate::mount::mnt::{Args, Fail, Mnt, Success};
use crate::mount::MountEntry;
use crate::rpc::AuthFlavor;
use crate::vfs::file;

use super::MountService;

fn make_file_handle(path: &file::Path) -> file::Handle {
    let mut handle = [0_u8; crate::nfsv3::NFS3_FHSIZE];
    let bytes = path.as_path().as_os_str().to_string_lossy().as_bytes().to_vec();

    if !bytes.is_empty() {
        for (idx, slot) in handle.iter_mut().enumerate() {
            *slot = bytes[idx % bytes.len()];
        }
    }

    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    let digest = hasher.finish().to_be_bytes();
    let offset = handle.len().saturating_sub(digest.len());
    handle[offset..].copy_from_slice(&digest);
    file::Handle(handle)
}

#[async_trait]
impl Mnt for MountService {
    async fn mnt(&mut self, args: Args, client_addr: SocketAddr) -> Result<Success, Fail> {
        let is_exported = self.exports.by_directory.contains_key(&args.dirpath);

        if !is_exported {
            return Err(Fail::Access);
        }

        let mount_entry =
            MountEntry { hostname: client_addr.ip().to_string(), directory: args.dirpath.clone() };

        self.mounts.by_client.entry(client_addr).or_default().insert(mount_entry);

        Ok(Success {
            file_handle: make_file_handle(&args.dirpath),
            auth_flavors: vec![AuthFlavor::None],
        })
    }
}
