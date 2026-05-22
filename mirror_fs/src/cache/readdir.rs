//! Caching for readdir operation results

use std::path::PathBuf;
use std::sync::Arc;

use moka::future::Cache;
use moka::policy::EvictionPolicy;
use nfs_mamont::vfs::file;
use nfs_mamont::vfs::file::Name;
const MAX_CACHE_CAPACITY: u64 = 64;

/// Snapshot of directory contents at a point in time
#[derive(Clone)]
pub struct DirectoryListingSnapshot {
    /// List of tuples (filename, full_path)
    pub entries: Arc<Vec<(Name, PathBuf)>>,
}

/// Cache for readdir operation results
///
/// Uses LRU eviction policy with support for entry invalidation.
pub struct ReadDirCache {
    cache: Cache<file::Handle, Arc<DirectoryListingSnapshot>>,
}

impl ReadDirCache {
    /// Creates a new empty cache
    pub fn new() -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(MAX_CACHE_CAPACITY)
                .eviction_policy(EvictionPolicy::lru())
                .support_invalidation_closures()
                .build(),
        }
    }

    /// Adds or updates cache entry for a directory
    pub async fn add_entry(&self, parent: &file::Handle, vec: Arc<DirectoryListingSnapshot>) {
        self.cache.insert(parent.clone(), vec).await
    }

    /// Invalidates all cache entries for directories containing files under the given path
    pub fn invalidate_entry(&self, dir: PathBuf) {
        // what should we do in case of fail?
        let _ = self.cache.invalidate_entries_if(move |_, entry| {
            entry.entries.iter().any(|(_, path)| path.starts_with(dir.as_path()))
        });
    }

    /// Looks up cache entry for a directory
    pub async fn look_for_cache(
        &self,
        parent: &file::Handle,
    ) -> Option<Arc<DirectoryListingSnapshot>> {
        self.cache.get(parent).await
    }
}
