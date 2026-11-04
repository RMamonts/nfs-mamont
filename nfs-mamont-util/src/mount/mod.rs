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
//! - active mounts are keyed by client socket address because one client can
//!   mount multiple directories and `UMNT`/`UMNTALL` are client-scoped;
//! - each export keeps server policy metadata (file handle + auth flavors)
//!   next to user-visible export data.
//!
//! Authentication decision:
//! - every export carries an [`ExportPolicy`]: the RPC flavors it accepts and,
//!   optionally, the `AUTH_SYS` uids and gids allowed to mount it;
//! - `MNT` is the single procedure with an error code, so it is the one place
//!   where the policy is enforced: a caller that fails any part of it gets
//!   `MNT3ERR_ACCES` and no mount is recorded;
//! - the flavor is checked first. [`AuthFlavor::None`] in a policy means "any
//!   flavor is accepted": this is how clients read a MOUNT reply listing
//!   `AUTH_NULL` (Linux keeps sending `AUTH_SYS` to such a server), so
//!   enforcement follows the same rule; an empty list denies every mount;
//! - uid/gid restrictions apply to the identity in an `AUTH_SYS` credential.
//!   An anonymous (`AUTH_NONE`) caller has no identity and is denied whenever
//!   a restriction is set. The identity is asserted by the client, which is
//!   the trust model of `sec=sys` as a whole: the same uid/gid drive file
//!   permission checks in the VFS, so the mount-time check is exactly as
//!   meaningful as the rest of NFSv3 without Kerberos;
//! - `EXPORT` and `DUMP` return public information and `UMNT`/`UMNTALL` are
//!   already scoped by the client address, none of them can report an error,
//!   so they ignore the credential;
//! - the check happens at mount time only: NFS operations on a file handle the
//!   client already holds are not gated here, that is the VFS layer's job.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use nfs_mamont::auth::{AuthSysParams, Credential};
use nfs_mamont::mount::{ExportEntry, MountEntry};
use nfs_mamont::vfs::file;
use nfs_mamont::AuthFlavor;
use tokio::sync::RwLock;

mod dump;
mod export;
mod mnt;
mod umnt;
mod umntall;

#[cfg(test)]
mod tests;

/// Who may mount an export.
///
/// Enforced by `MNT`; see the module docs for the exact rules.
#[derive(Clone, Debug)]
pub struct ExportPolicy {
    /// RPC flavors advertised in the `MNT` reply and enforced on the call, in
    /// preference order. [`AuthFlavor::None`] in the list accepts any flavor;
    /// an empty list denies every mount.
    ///
    /// Should be a subset of [`SUPPORTED_AUTH_FLAVORS`] (pass that list as-is
    /// to accept everything the server can decode): a flavor outside it is
    /// advertised as given, but every call using it is rejected by the RPC
    /// layer before it reaches any service.
    ///
    /// [`SUPPORTED_AUTH_FLAVORS`]: nfs_mamont::auth::SUPPORTED_AUTH_FLAVORS
    pub auth_flavors: Vec<AuthFlavor>,
    /// `AUTH_SYS` uids allowed to mount. `None` means any uid.
    ///
    /// `Some(_)` makes an identity mandatory: anonymous callers are denied.
    pub allowed_uids: Option<HashSet<u32>>,
    /// Groups allowed to mount: the caller's primary gid or any of its
    /// auxiliary gids must be listed. `None` means any group.
    ///
    /// `Some(_)` makes an identity mandatory: anonymous callers are denied.
    pub allowed_gids: Option<HashSet<u32>>,
}

/// Why a caller was refused by an [`ExportPolicy`]; logged by `MNT`.
#[derive(Debug)]
enum Denial {
    /// The credential flavor is not in `auth_flavors`.
    Flavor,
    /// The policy restricts uids or gids but the caller is anonymous.
    Anonymous,
    /// The caller's uid is not in `allowed_uids`.
    Uid,
    /// Neither the caller's gid nor any of its auxiliary gids is in `allowed_gids`.
    Gid,
}

impl ExportPolicy {
    /// Policy accepting the given flavors with no identity restrictions.
    pub fn new(auth_flavors: Vec<AuthFlavor>) -> Self {
        Self { auth_flavors, allowed_uids: None, allowed_gids: None }
    }

    /// Checks `cred` against the policy.
    fn check(&self, cred: &Credential) -> Result<(), Denial> {
        let flavor = cred.flavor();
        if !self.auth_flavors.iter().any(|f| *f == AuthFlavor::None || *f == flavor) {
            return Err(Denial::Flavor);
        }

        if self.allowed_uids.is_none() && self.allowed_gids.is_none() {
            return Ok(());
        }

        let params: &AuthSysParams = match cred {
            Credential::Sys(params) => params,
            Credential::None => return Err(Denial::Anonymous),
        };

        if let Some(uids) = &self.allowed_uids {
            if !uids.contains(&params.uid) {
                return Err(Denial::Uid);
            }
        }

        if let Some(gids) = &self.allowed_gids {
            let in_group =
                gids.contains(&params.gid) || params.gids.iter().any(|g| gids.contains(g));
            if !in_group {
                return Err(Denial::Gid);
            }
        }

        Ok(())
    }
}

/// Export entry together with the server policy applied by `MNT`.
#[derive(Clone)]
struct ExportEntryWrapper {
    export: ExportEntry,
    root_handle: file::Handle,
    policy: ExportPolicy,
}

/// Registry of exported directories advertised by the server
#[derive(Default)]
struct ExportRegistry {
    /// A single directory has at most one export entry
    by_directory: HashMap<file::Path, ExportEntryWrapper>,
}

impl ExportRegistry {
    fn by_path(&self, path: &file::Path) -> Option<&ExportEntryWrapper> {
        self.by_directory.get(path)
    }

    fn insert(&mut self, directory: file::Path, root_handle: file::Handle, policy: ExportPolicy) {
        self.by_directory.insert(
            directory.clone(),
            ExportEntryWrapper {
                export: ExportEntry { directory, names: Vec::new() },
                root_handle,
                policy,
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

/// Registry of active mounts grouped by client endpoint
#[derive(Default)]
struct MountRegistry {
    /// A single client may mount multiple directories
    by_client: HashMap<SocketAddr, HashSet<MountEntry>>,
}

/// In-memory state backing the MOUNT v3 service implementation
pub struct MountService {
    /// Exported directories that are available for mounting
    exports: RwLock<ExportRegistry>,
    /// Active mounts keyed by client.
    mounts: RwLock<MountRegistry>,
}

impl Default for MountService {
    /// Creates a [`MountService`] with no exports and no active mounts.
    fn default() -> Self {
        Self {
            exports: RwLock::new(ExportRegistry::default()),
            mounts: RwLock::new(MountRegistry::default()),
        }
    }
}

impl MountService {
    /// Creates a [`MountService`] with no exports and no active mounts.
    ///
    /// Exports are registered with [`MountService::add_export`].
    pub fn new() -> Self {
        Self::default()
    }

    async fn export_entry(&self, path: &file::Path) -> Option<ExportEntryWrapper> {
        self.exports.read().await.by_path(path).cloned()
    }

    /// Add a new export entry at runtime.
    ///
    /// `policy` decides who may mount the export; `MNT` enforces it and
    /// advertises its `auth_flavors` to the client. See [`ExportPolicy`].
    ///
    /// If an export for `directory` already exists it will be replaced.
    pub async fn add_export(
        &self,
        directory: file::Path,
        root_handle: file::Handle,
        policy: ExportPolicy,
    ) {
        self.exports.write().await.insert(directory, root_handle, policy);
    }

    /// Remove an export entry at runtime.
    ///
    /// Returns `true` if the export existed and was removed.
    pub async fn remove_export(&self, directory: &file::Path) -> bool {
        self.exports.write().await.remove(directory)
    }
}
