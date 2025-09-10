use nfs_mamont::xdr::nfs3::nfsstat3;

/// Result type for NFS operations
pub type NFSResult<T> = Result<T, nfsstat3>;

/// Enum for refresh results
pub enum RefreshResult {
    /// The fileid was deleted
    Delete,
    /// The fileid needs to be reloaded. mtime has been updated, caches
    /// need to be evicted.
    Reload,
    /// Nothing has changed
    Noop,
}
