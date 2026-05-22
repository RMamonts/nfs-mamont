use moka::future::Cache;
use moka::policy::EvictionPolicy;
use nfs_mamont::vfs::file::Name;
use nfs_mamont::vfs::{file, read_dir};
use std::path::PathBuf;
use std::sync::Arc;

const MAX_CACHE_CAPACITY: u64 = 64;

pub struct DirectoryListingSnapshot {
    pub verifier: read_dir::CookieVerifier,
    pub entries: Arc<Vec<(Name, PathBuf)>>,
}

pub struct ReadDirCache {
    cache: Cache<file::Handle, Arc<DirectoryListingSnapshot>>,
}

impl ReadDirCache {
    pub fn new() -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(MAX_CACHE_CAPACITY)
                .eviction_policy(EvictionPolicy::lru())
                .support_invalidation_closures()
                .build(),
        }
    }

    pub async fn add_entry(&self, parent: &file::Handle, vec: Arc<DirectoryListingSnapshot>) {
        self.cache.insert(parent.clone(), vec).await
    }

    pub fn invalidate_entry(&self, dir: PathBuf) {
        self.cache.invalidate_entries_if(move |_, entry| {
            entry.entries.iter().any(|(_, path)| path.starts_with(dir.as_path()))
        });
    }

    pub async fn look_for_cache(
        &self,
        parent: &file::Handle,
    ) -> Option<Arc<DirectoryListingSnapshot>> {
        self.cache.get(parent).await
    }
}
