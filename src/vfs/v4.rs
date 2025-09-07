use crate::vfs::v3::Capabilities;
use crate::xdr::nfs4;
use async_trait::async_trait;

/// Defines the core interface for an NFSv4 filesystem implementation.
/// This trait must be implemented by any backend storage system that wants to support NFSv4 protocol.
#[async_trait]
pub trait NFSv4FileSystem: Sync {
    /// Returns the current generation number of the filesystem.
    /// This number should change on each filesystem state change (e.g., reboot, remount)
    /// and is used to detect stale state handles from clients after server restart.
    fn generation(&self) -> u64;

    /// Returns the file ID of the root directory of the filesystem.
    /// This is the entry point for all path resolution operations.
    fn root_dir(&self) -> nfs4::fileid4;

    /// Returns the capabilities supported by this filesystem implementation.
    fn capabilities(&self) -> Capabilities;

    /// Retrieves attributes for a given file or directory identified by its file ID.
    /// Returns a filled fattr4 structure containing the requested file attributes,
    /// or an NFS error code if the file doesn't exist or access is denied.
    async fn getattr(&self, id: nfs4::fileid4) -> Result<nfs4::fattr4, nfs4::nfsstat4>;
}
