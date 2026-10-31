//! Server-side state and handlers for the MOUNT v3 RPC program.
//!
//! The structures in this module keep track of two related views:
//! exported directories available for mounting and currently active mounts
//! reported by clients.
//!
//! Mount/filehandle resolution decision:
//! - MOUNT `MNT` returns only initial filehandles for explicitly exported paths;
//! - path traversal inside mounted subtree is expected to go through NFS `LOOKUP`;
//! - therefore MOUNT service owns mapping only for mountable roots, while regular
//!   filename-to-filehandle resolution belongs to VFS/NFS layer;
//! - this mirrors common access policy where clients are granted a specific export
//!   subtree and should not rely on walking to upper directories via MOUNT.
//!
//! State structure follows this split:
//! - exports are keyed by directory path for direct `MNT` lookup;
//! - active mounts are keyed by client IP address because one client can
//!   mount multiple directories and `UMNT`/`UMNTALL` are client-scoped; the
//!   port is deliberately not part of the key, since `mount` and `umount` are
//!   separate processes that reach the server over separate TCP connections;
//! - each export keeps server policy metadata (file handle + auth flavors)
//!   next to user-visible export data.

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;

use tokio::sync::RwLock;

use crate::mount::{ExportEntry, MountEntry};
use crate::rpc::AuthFlavor;
use crate::vfs::file;

mod dump;
mod export;
mod mnt;
mod umnt;
mod umntall;

// TODO: should be taken from config
const AUTH: [AuthFlavor; 1] = [AuthFlavor::None];

#[derive(Clone)]
pub struct ExportEntryWrapper {
    pub export: ExportEntry,
    pub root_handle: file::Handle,
}

/// Registry of exported directories advertised by the server
#[derive(Default)]
struct ExportRegistry {
    /// A single directory has at most one export entry
    by_directory: HashMap<file::Path, ExportEntryWrapper>,
}

impl ExportRegistry {
    fn from_entries(entries: Vec<ExportEntryWrapper>) -> Self {
        let mut by_directory = HashMap::new();
        for entry in entries {
            by_directory.insert(entry.export.directory.clone(), entry);
        }
        Self { by_directory }
    }

    fn by_path(&self, path: &file::Path) -> Option<&ExportEntryWrapper> {
        self.by_directory.get(path)
    }

    fn insert(&mut self, directory: file::Path, root_handle: file::Handle) {
        self.by_directory.insert(
            directory.clone(),
            ExportEntryWrapper {
                export: ExportEntry { directory, names: Vec::new() },
                root_handle,
            },
        );
    }

    fn remove(&mut self, directory: &file::Path) -> bool {
        self.by_directory.remove(directory).is_some()
    }

    fn export_list(&self) -> Vec<ExportEntry> {
        self.by_directory.values().map(|entry| entry.export.clone()).collect()
    }
}

/// Registry of active mounts grouped by client host.
///
/// Keyed by IP address only: `MNT`, `UMNT` and `UMNTALL` for one host arrive
/// over different TCP connections with different source ports, so a key that
/// included the port would never match an existing entry. Addresses are stored
/// in canonical form ([`IpAddr::to_canonical`]): a dual-stack listener reports
/// IPv4 peers as IPv4-mapped IPv6 (`::ffff:a.b.c.d`), which is folded back to
/// plain IPv4 so one host always has one key.
#[derive(Default)]
struct MountRegistry {
    /// A single client may mount multiple directories
    by_client: HashMap<IpAddr, HashSet<MountEntry>>,
}

/// In-memory state backing the MOUNT v3 service implementation
pub struct MountService {
    /// Exported directories that are available for mounting
    exports: RwLock<ExportRegistry>,
    /// Active mounts keyed by client.
    mounts: RwLock<MountRegistry>,
}

impl MountService {
    pub fn with_exports(entries: Vec<ExportEntryWrapper>) -> Self {
        Self {
            exports: RwLock::new(ExportRegistry::from_entries(entries)),
            mounts: RwLock::new(MountRegistry::default()),
        }
    }

    async fn export_entry(&self, path: &file::Path) -> Option<ExportEntryWrapper> {
        self.exports.read().await.by_path(path).cloned()
    }

    /// Add a new export entry at runtime.
    ///
    /// If an export for `directory` already exists it will be replaced.
    pub async fn add_export(&self, directory: file::Path, root_handle: file::Handle) {
        self.exports.write().await.insert(directory, root_handle);
    }

    /// Remove an export entry at runtime.
    ///
    /// Returns `true` if the export existed and was removed.
    pub async fn remove_export(&self, directory: &file::Path) -> bool {
        self.exports.write().await.remove(directory)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use super::*;
    use crate::consts::nfsv3::NFS3_FHSIZE;
    use crate::mount::dump::Dump;
    use crate::mount::mnt::{self, Mnt};
    use crate::mount::umnt::{self, Umnt};
    use crate::mount::umntall::Umntall;
    use crate::rpc::auth::Credential;
    use crate::vfs::file;

    fn handle(byte: u8) -> file::Handle {
        file::Handle([byte; NFS3_FHSIZE])
    }

    fn path(name: &str) -> file::Path {
        file::Path::new(name.to_string()).unwrap()
    }

    fn client(port: u16) -> SocketAddr {
        SocketAddr::new(Ipv4Addr::new(10, 0, 0, 7).into(), port)
    }

    async fn mounted_dirs(service: &MountService) -> Vec<file::Path> {
        let mut dirs = service
            .dump(&Credential::None)
            .await
            .mount_list
            .into_iter()
            .map(|entry| entry.directory)
            .collect::<Vec<_>>();
        dirs.sort_by(|a, b| a.as_path().cmp(b.as_path()));
        dirs
    }

    /// `mount` and `umount` are separate processes on the client, so UMNT
    /// arrives over a different connection (different source port) than MNT did.
    #[tokio::test]
    async fn umnt_from_another_port_of_the_same_host_removes_the_mount() {
        let service = service();

        service
            .mnt(mnt::Args { dirpath: path("/initial") }, client(1000), &Credential::None)
            .await
            .unwrap();
        assert_eq!(mounted_dirs(&service).await, vec![path("/initial")]);

        service
            .umnt(umnt::Args { dirpath: path("/initial") }, client(2000), &Credential::None)
            .await;

        assert!(mounted_dirs(&service).await.is_empty());
        assert!(service.mounts.read().await.by_client.is_empty());
    }

    #[tokio::test]
    async fn umntall_from_another_port_of_the_same_host_removes_all_mounts() {
        let service = service();
        service.add_export(path("/second"), handle(2)).await;

        service
            .mnt(mnt::Args { dirpath: path("/initial") }, client(1000), &Credential::None)
            .await
            .unwrap();
        service
            .mnt(mnt::Args { dirpath: path("/second") }, client(1001), &Credential::None)
            .await
            .unwrap();
        assert_eq!(mounted_dirs(&service).await, vec![path("/initial"), path("/second")]);

        service.umntall(client(2000), &Credential::None).await;

        assert!(mounted_dirs(&service).await.is_empty());
    }

    #[tokio::test]
    async fn umnt_from_another_host_keeps_the_mount() {
        let service = service();
        let other_host = SocketAddr::new(Ipv4Addr::new(10, 0, 0, 8).into(), 1000);

        service
            .mnt(mnt::Args { dirpath: path("/initial") }, client(1000), &Credential::None)
            .await
            .unwrap();

        service.umnt(umnt::Args { dirpath: path("/initial") }, other_host, &Credential::None).await;

        assert_eq!(mounted_dirs(&service).await, vec![path("/initial")]);
    }

    /// A dual-stack listener reports IPv4 peers as IPv4-mapped IPv6 addresses;
    /// they must be the same host as the plain IPv4 form.
    #[tokio::test]
    async fn ipv4_mapped_client_is_the_same_host_as_plain_ipv4() {
        let service = service();
        let mapped = SocketAddr::new(Ipv4Addr::new(10, 0, 0, 7).to_ipv6_mapped().into(), 1000);

        service
            .mnt(mnt::Args { dirpath: path("/initial") }, mapped, &Credential::None)
            .await
            .unwrap();

        let mount_list = service.dump(&Credential::None).await.mount_list;
        assert_eq!(mount_list.len(), 1);
        assert_eq!(mount_list[0].hostname.as_str(), "10.0.0.7");

        service
            .umnt(umnt::Args { dirpath: path("/initial") }, client(2000), &Credential::None)
            .await;

        assert!(mounted_dirs(&service).await.is_empty());
    }

    fn service() -> MountService {
        MountService::with_exports(vec![ExportEntryWrapper {
            export: ExportEntry { directory: path("/initial"), names: Vec::new() },
            root_handle: handle(1),
        }])
    }

    #[tokio::test]
    async fn with_exports_keeps_initial_entries() {
        let service = service();

        let entry = service.export_entry(&path("/initial")).await.unwrap();
        assert_eq!(entry.root_handle, handle(1));
        assert_eq!(service.exports.read().await.export_list().len(), 1);
    }

    #[tokio::test]
    async fn add_export_makes_entry_visible() {
        let service = service();

        service.add_export(path("/added"), handle(2)).await;

        let entry = service.export_entry(&path("/added")).await.unwrap();
        assert_eq!(entry.root_handle, handle(2));
        let directories = service
            .exports
            .read()
            .await
            .export_list()
            .into_iter()
            .map(|entry| entry.directory)
            .collect::<Vec<_>>();
        assert_eq!(directories.len(), 2);
        assert!(directories.contains(&path("/added")));
    }

    #[tokio::test]
    async fn add_export_replaces_existing_entry() {
        let service = service();

        service.add_export(path("/initial"), handle(9)).await;

        let entry = service.export_entry(&path("/initial")).await.unwrap();
        assert_eq!(entry.root_handle, handle(9));
        assert_eq!(service.exports.read().await.export_list().len(), 1);
    }

    #[tokio::test]
    async fn remove_export_removes_entry_and_reports_whether_it_existed() {
        let service = service();

        assert!(service.remove_export(&path("/initial")).await);
        assert!(!service.remove_export(&path("/initial")).await);
        assert!(service.exports.read().await.export_list().is_empty());
    }
}
