use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::fs::Metadata;
use std::io::ErrorKind;
use std::os::unix::fs::DirEntryExt;
use std::os::unix::fs::{FileExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use moka::sync::Cache;
use tokio::sync::RwLock;
use whirlwind::ShardSet;

use nfs_mamont::consts::nfsv3::{NFS3_COOKIEVERFSIZE, NFS3_CREATEVERFSIZE};
use nfs_mamont::vfs;
use nfs_mamont::vfs::file;
use nfs_mamont::vfs::read_dir;
use nfs_mamont::vfs::set_attr;
use nfs_mamont::vfs::write;

use crate::fs_map::FsMap;

mod access_impl;
mod commit_impl;
mod create_impl;
mod fs_info_impl;
mod fs_stat_impl;
mod get_attr_impl;
mod link_impl;
mod lookup_impl;
mod mk_dir_impl;
mod mk_node_impl;
mod path_conf_impl;
mod read_dir_impl;
mod read_dir_plus_impl;
mod read_impl;
mod read_link_impl;
mod remove_impl;
mod rename_impl;
mod rm_dir_impl;
mod set_attr_impl;
mod symlink_impl;
mod write_impl;

pub const READ_WRITE_MAX: u32 = 1024 * 1024;
const READ_DIR_PREF: u32 = 8 * 1024;
const READ_FILE_CACHE_LIMIT: usize = 1024;
const ATTRIBUTE_CACHE_LIMIT: usize = 16 * 1024;
const ATTRIBUTE_CACHE_TTL: Duration = Duration::from_secs(60);
const DIRECTORY_LISTING_CACHE_LIMIT: usize = 1024;
const DIRECTORY_LISTING_CACHE_TTL: Duration = Duration::from_millis(700);
const READ_AHEAD_BLOCK_SIZE: usize = READ_WRITE_MAX as usize;
const READ_AHEAD_WINDOW_BLOCKS: usize = 8;
const READ_AHEAD_CACHE_LIMIT: usize = 1024;
const READ_AHEAD_PER_FILE_LIMIT: usize = 16;
const READ_AHEAD_CACHE_TTL: Duration = Duration::from_secs(30);
const READ_AHEAD_SEQUENCE_WINDOW: Duration = Duration::from_secs(6);
const READDIRPLUS_ATTR_PARALLELISM: usize = 16;
const EXPORT_ID_SHIFT: u32 = 56;
const LOCAL_HANDLE_MASK: u64 = (1u64 << EXPORT_ID_SHIFT) - 1;
const DEFAULT_SET_ATTR: set_attr::NewAttr = set_attr::NewAttr {
    mode: None,
    uid: None,
    gid: None,
    size: None,
    atime: set_attr::SetTime::DontChange,
    mtime: set_attr::SetTime::DontChange,
};

/// A file system implementation that mirrors a local directory.
pub struct MirrorFS {
    exports: Vec<ExportState>,
    read_file_cache: ReadFileCache,
    write_file_cache: WriteFileCache,
    attr_cache: AttributeCache,
    dir_listing_cache: DirectoryListingCache,
    read_ahead_cache: ReadAheadCache,
    pending_unstable_writes: Arc<tokio::sync::Mutex<HashSet<PathBuf>>>,
    generation: u64,
}

struct ExportState {
    root_path: PathBuf,
    fsmap: FsMap,
}

#[derive(Clone)]
struct DirectoryEntrySnapshot {
    name: file::Name,
    path: PathBuf,
    file_id: u64,
}

struct DirectoryListingSnapshot {
    verifier: read_dir::CookieVerifier,
    entries: Arc<[DirectoryEntrySnapshot]>,
}

struct DirectoryListingCache {
    listings: Cache<PathBuf, Arc<DirectoryListingSnapshot>>,
}

struct ReadFileCache {
    files: Cache<PathBuf, Arc<File>>,
    keys: ShardSet<PathBuf>,
    key_index: RwLock<BTreeSet<PathBuf>>,
}

struct WriteFileCache {
    files: Cache<PathBuf, Arc<File>>,
    keys: ShardSet<PathBuf>,
    key_index: RwLock<BTreeSet<PathBuf>>,
}

#[derive(Debug)]
struct CachedAttrEntry {
    metadata: Metadata,
}

struct AttributeCache {
    attrs: Cache<PathBuf, Arc<CachedAttrEntry>>,
    keys: ShardSet<PathBuf>,
    key_index: RwLock<BTreeSet<PathBuf>>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ReadAheadKey {
    path: PathBuf,
    block_index: u64,
}

#[derive(Clone, Copy, Debug)]
struct ReadSequenceState {
    next_offset: u64,
    last_seen: Instant,
}

#[derive(Clone)]
struct ReadAheadCache {
    blocks: Cache<ReadAheadKey, Arc<[u8]>>,
    key_index: Arc<RwLock<BTreeSet<ReadAheadKey>>>,
    inflight: Arc<tokio::sync::Mutex<HashSet<ReadAheadKey>>>,
    sequence: Arc<tokio::sync::Mutex<HashMap<PathBuf, ReadSequenceState>>>,
}

impl ReadFileCache {
    fn new() -> Self {
        Self {
            files: Cache::builder().max_capacity(READ_FILE_CACHE_LIMIT as u64).build(),
            keys: ShardSet::new(),
            key_index: RwLock::new(BTreeSet::new()),
        }
    }
}

impl WriteFileCache {
    fn new() -> Self {
        Self {
            files: Cache::builder().max_capacity(READ_FILE_CACHE_LIMIT as u64).build(),
            keys: ShardSet::new(),
            key_index: RwLock::new(BTreeSet::new()),
        }
    }
}

impl AttributeCache {
    fn new() -> Self {
        Self {
            attrs: Cache::builder()
                .max_capacity(ATTRIBUTE_CACHE_LIMIT as u64)
                .time_to_live(ATTRIBUTE_CACHE_TTL)
                .build(),
            keys: ShardSet::new(),
            key_index: RwLock::new(BTreeSet::new()),
        }
    }
}

impl DirectoryListingCache {
    fn new() -> Self {
        Self {
            listings: Cache::builder()
                .max_capacity(DIRECTORY_LISTING_CACHE_LIMIT as u64)
                .time_to_live(DIRECTORY_LISTING_CACHE_TTL)
                .build(),
        }
    }
}

impl ReadAheadCache {
    fn new() -> Self {
        Self {
            blocks: Cache::builder()
                .max_capacity(READ_AHEAD_CACHE_LIMIT as u64)
                .time_to_live(READ_AHEAD_CACHE_TTL)
                .build(),
            key_index: Arc::new(RwLock::new(BTreeSet::new())),
            inflight: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            sequence: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    async fn maybe_compact_keys(&self) {
        if self.key_index.read().await.len() <= READ_AHEAD_CACHE_LIMIT * 4 {
            return;
        }

        let keys: Vec<ReadAheadKey> = self.key_index.read().await.iter().cloned().collect();
        let stale: Vec<ReadAheadKey> =
            keys.into_iter().filter(|key| self.blocks.get(key).is_none()).collect();

        for key in stale {
            self.key_index.write().await.remove(&key);
        }
    }
}

impl MirrorFS {
    /// Creates a new mirror file system with the given root path.
    pub fn new(root: PathBuf) -> Self {
        Self::new_many(vec![root])
    }

    /// Creates a new mirror file system with multiple export roots.
    pub fn new_many(roots: Vec<PathBuf>) -> Self {
        assert!(!roots.is_empty(), "mirror fs requires at least one export");
        assert!(roots.len() <= 256, "mirror fs supports at most 256 exports");
        let exports = roots
            .into_iter()
            .map(|root| {
                let root = std::fs::canonicalize(&root).unwrap_or(root);
                ExportState { fsmap: FsMap::new(root.clone()), root_path: root }
            })
            .collect();
        let generation =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_nanos()
                as u64;
        Self {
            exports,
            read_file_cache: ReadFileCache::new(),
            write_file_cache: WriteFileCache::new(),
            attr_cache: AttributeCache::new(),
            dir_listing_cache: DirectoryListingCache::new(),
            read_ahead_cache: ReadAheadCache::new(),
            pending_unstable_writes: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            generation,
        }
    }

    /// Returns the root handle.
    pub async fn root_handle(&self) -> file::Handle {
        self.root_handle_for_export(0).await
    }

    /// Returns the root handle for the requested export.
    pub async fn root_handle_for_export(&self, export_id: usize) -> file::Handle {
        Self::compose_handle(export_id, self.exports[export_id].fsmap.root_handle())
    }

    fn write_verifier(&self) -> write::Verifier {
        write::Verifier(self.generation.to_be_bytes())
    }

    fn cookie_verifier_for_attr(attr: &file::Attr) -> read_dir::CookieVerifier {
        let mut raw = [0u8; NFS3_COOKIEVERFSIZE];
        raw[..4].copy_from_slice(&attr.ctime.seconds.to_be_bytes());
        raw[4..].copy_from_slice(&attr.ctime.nanos.to_be_bytes());
        read_dir::CookieVerifier::new(raw)
    }

    async fn path_for_handle(&self, handle: &file::Handle) -> Result<PathBuf, vfs::Error> {
        self.path_for_handle_with_export(handle).await.map(|(_, path)| path)
    }

    async fn path_for_handle_with_export(
        &self,
        handle: &file::Handle,
    ) -> Result<(usize, PathBuf), vfs::Error> {
        self.path_and_attr_for_handle(handle).await.map(|(export_id, path, _)| (export_id, path))
    }

    async fn path_and_attr_for_handle(
        &self,
        handle: &file::Handle,
    ) -> Result<(usize, PathBuf, file::Attr), vfs::Error> {
        let (export_id, inner_handle) = self.split_handle(handle)?;
        let export = self.export_state(export_id)?;
        let candidates = export.fsmap.path_candidates_for_handle(&inner_handle).await?;
        for candidate in candidates {
            if let Ok(attr) = self.attr_for_path(&candidate).await {
                return Ok((export_id, candidate, attr));
            }
        }
        Err(vfs::Error::StaleFile)
    }

    async fn ensure_handle_for_path(
        &self,
        export_id: usize,
        path: &Path,
    ) -> Result<file::Handle, vfs::Error> {
        let attr = self.attr_for_path(path).await?;
        let export = self.export_state(export_id)?;
        let inner_handle = export.fsmap.ensure_handle_for_attr(path, &attr).await?;
        Ok(Self::compose_handle(export_id, inner_handle))
    }

    async fn ensure_handles_for_paths(
        &self,
        export_id: usize,
        paths: &[PathBuf],
    ) -> Result<Vec<file::Handle>, vfs::Error> {
        let mut handles = Vec::with_capacity(paths.len());
        for path in paths {
            handles.push(self.ensure_handle_for_path(export_id, path).await?);
        }
        Ok(handles)
    }

    async fn cache_handles_for_paths(&self, export_id: usize, paths: &[PathBuf]) {
        for path in paths {
            let _ = self.ensure_handle_for_path(export_id, path).await;
        }
    }

    async fn remove_cached_path(&self, export_id: usize, path: &Path) {
        if let Ok(export) = self.export_state(export_id) {
            export.fsmap.remove_path(path).await;
        }
        self.invalidate_read_file_cache_path(path).await;
        self.invalidate_write_file_cache_path(path).await;
        self.invalidate_attr_cache_path(path).await;
        self.invalidate_read_ahead_path(path).await;
        self.clear_pending_unstable_write(path).await;
    }

    async fn rename_cached_path(
        &self,
        export_id: usize,
        from: &Path,
        to: &Path,
    ) -> Result<(), vfs::Error> {
        let export = self.export_state(export_id)?;
        export.fsmap.rename_path(from, to).await?;
        self.invalidate_read_file_cache_path(from).await;
        self.invalidate_read_file_cache_path(to).await;
        self.invalidate_write_file_cache_path(from).await;
        self.invalidate_write_file_cache_path(to).await;
        self.invalidate_attr_cache_path(from).await;
        self.invalidate_attr_cache_path(to).await;
        self.invalidate_read_ahead_path(from).await;
        self.invalidate_read_ahead_path(to).await;
        self.clear_pending_unstable_write(from).await;
        self.clear_pending_unstable_write(to).await;
        Ok(())
    }

    async fn mark_pending_unstable_write(&self, path: &Path) {
        self.pending_unstable_writes.lock().await.insert(path.to_path_buf());
    }

    async fn clear_pending_unstable_write(&self, path: &Path) {
        self.pending_unstable_writes.lock().await.remove(path);
    }

    async fn attr_for_path(&self, path: &Path) -> Result<file::Attr, vfs::Error> {
        if let Some(entry) = self.attr_cache.attrs.get(path) {
            return Ok(Self::attr_from_metadata(&entry.metadata));
        }
        self.attr_cache.keys.remove(&path.to_path_buf()).await;
        self.attr_cache.key_index.write().await.remove(path);

        let path_buf = path.to_path_buf();
        let metadata = tokio::task::spawn_blocking(move || {
            std::fs::symlink_metadata(path_buf).map_err(|error| Self::io_error_to_vfs(&error))
        })
        .await
        .map_err(|_| vfs::Error::IO)??;

        let attr = Self::attr_from_metadata(&metadata);

        let key = path.to_path_buf();
        self.attr_cache.attrs.insert(key.clone(), Arc::new(CachedAttrEntry { metadata }));
        self.attr_cache.keys.insert(key.clone()).await;
        self.attr_cache.key_index.write().await.insert(key);
        self.maybe_compact_attr_keys().await;

        Ok(attr)
    }

    async fn get_cached_read_file(&self, path: &Path) -> Result<Arc<File>, vfs::Error> {
        if let Some(file) = self.read_file_cache.files.get(path) {
            return Ok(file);
        }

        let path_buf = path.to_path_buf();
        let opened = tokio::task::spawn_blocking(move || File::open(path_buf))
            .await
            .map_err(|_| vfs::Error::IO)?
            .map_err(|error| Self::io_error_to_vfs(&error))?;
        let opened = Arc::new(opened);

        if let Some(existing) = self.read_file_cache.files.get(path) {
            return Ok(existing);
        }

        let key = path.to_path_buf();
        self.read_file_cache.files.insert(key.clone(), opened.clone());
        self.read_file_cache.keys.insert(key.clone()).await;
        self.read_file_cache.key_index.write().await.insert(key);
        self.maybe_compact_read_file_keys().await;

        Ok(opened)
    }

    async fn get_cached_write_file(&self, path: &Path) -> Result<Arc<File>, vfs::Error> {
        if let Some(file) = self.write_file_cache.files.get(path) {
            return Ok(file);
        }

        let path_buf = path.to_path_buf();
        let opened = tokio::task::spawn_blocking(move || {
            std::fs::OpenOptions::new().write(true).truncate(false).open(path_buf)
        })
        .await
        .map_err(|_| vfs::Error::IO)?
        .map_err(|error| Self::io_error_to_vfs(&error))?;
        let opened = Arc::new(opened);

        if let Some(existing) = self.write_file_cache.files.get(path) {
            return Ok(existing);
        }

        let key = path.to_path_buf();
        self.write_file_cache.files.insert(key.clone(), opened.clone());
        self.write_file_cache.keys.insert(key.clone()).await;
        self.write_file_cache.key_index.write().await.insert(key);
        self.maybe_compact_write_file_keys().await;

        Ok(opened)
    }

    async fn invalidate_read_file_cache_path(&self, path: &Path) {
        let keys: Vec<PathBuf> = {
            let index = self.read_file_cache.key_index.read().await;
            index
                .iter()
                .filter(|known_path| *known_path == path || known_path.starts_with(path))
                .cloned()
                .collect()
        };

        for key in keys {
            self.read_file_cache.files.invalidate(&key);
            self.read_file_cache.keys.remove(&key).await;
            self.read_file_cache.key_index.write().await.remove(&key);
        }
    }

    async fn invalidate_write_file_cache_path(&self, path: &Path) {
        let keys: Vec<PathBuf> = {
            let index = self.write_file_cache.key_index.read().await;
            index
                .iter()
                .filter(|known_path| *known_path == path || known_path.starts_with(path))
                .cloned()
                .collect()
        };

        for key in keys {
            self.write_file_cache.files.invalidate(&key);
            self.write_file_cache.keys.remove(&key).await;
            self.write_file_cache.key_index.write().await.remove(&key);
        }
    }

    async fn invalidate_attr_cache_path(&self, path: &Path) {
        let keys: Vec<PathBuf> = {
            let index = self.attr_cache.key_index.read().await;
            index
                .iter()
                .filter(|known_path| *known_path == path || known_path.starts_with(path))
                .cloned()
                .collect()
        };
        for key in keys {
            self.attr_cache.attrs.invalidate(&key);
            self.attr_cache.keys.remove(&key).await;
            self.attr_cache.key_index.write().await.remove(&key);
        }
    }

    async fn store_attr_cache_metadata(&self, path: PathBuf, metadata: Metadata) {
        self.attr_cache.attrs.insert(path.clone(), Arc::new(CachedAttrEntry { metadata }));
        self.attr_cache.keys.insert(path.clone()).await;
        self.attr_cache.key_index.write().await.insert(path);
        self.maybe_compact_attr_keys().await;
    }

    async fn maybe_compact_read_file_keys(&self) {
        if self.read_file_cache.key_index.read().await.len() <= READ_FILE_CACHE_LIMIT * 4 {
            return;
        }
        let keys: Vec<PathBuf> =
            self.read_file_cache.key_index.read().await.iter().cloned().collect();
        let stale_keys: Vec<PathBuf> =
            keys.into_iter().filter(|key| self.read_file_cache.files.get(key).is_none()).collect();
        for key in stale_keys {
            self.read_file_cache.keys.remove(&key).await;
            self.read_file_cache.key_index.write().await.remove(&key);
        }
    }

    async fn maybe_compact_write_file_keys(&self) {
        if self.write_file_cache.key_index.read().await.len() <= READ_FILE_CACHE_LIMIT * 4 {
            return;
        }
        let keys: Vec<PathBuf> =
            self.write_file_cache.key_index.read().await.iter().cloned().collect();
        let stale_keys: Vec<PathBuf> =
            keys.into_iter().filter(|key| self.write_file_cache.files.get(key).is_none()).collect();
        for key in stale_keys {
            self.write_file_cache.keys.remove(&key).await;
            self.write_file_cache.key_index.write().await.remove(&key);
        }
    }

    async fn maybe_compact_attr_keys(&self) {
        if self.attr_cache.key_index.read().await.len() <= ATTRIBUTE_CACHE_LIMIT * 4 {
            return;
        }
        let keys: Vec<PathBuf> = self.attr_cache.key_index.read().await.iter().cloned().collect();
        let stale_keys: Vec<PathBuf> =
            keys.into_iter().filter(|key| self.attr_cache.attrs.get(key).is_none()).collect();
        for key in stale_keys {
            self.attr_cache.keys.remove(&key).await;
            self.attr_cache.key_index.write().await.remove(&key);
        }
    }

    async fn attrs_for_paths_parallel(
        &self,
        paths: &[PathBuf],
    ) -> Result<Vec<file::Attr>, vfs::Error> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        let mut attrs: Vec<Option<file::Attr>> = (0..paths.len()).map(|_| None).collect();
        let mut misses: Vec<(usize, PathBuf)> = Vec::new();

        for (idx, path) in paths.iter().enumerate() {
            if let Some(entry) = self.attr_cache.attrs.get(path) {
                attrs[idx] = Some(Self::attr_from_metadata(&entry.metadata));
                continue;
            }

            self.attr_cache.keys.remove(path).await;
            self.attr_cache.key_index.write().await.remove(path);
            misses.push((idx, path.clone()));
        }

        for batch in misses.chunks(READDIRPLUS_ATTR_PARALLELISM) {
            let mut tasks = tokio::task::JoinSet::new();

            for (idx, path) in batch.iter().cloned() {
                tasks.spawn(async move {
                    let path_for_meta = path.clone();
                    let metadata = tokio::task::spawn_blocking(move || {
                        std::fs::symlink_metadata(path_for_meta)
                    })
                    .await
                    .map_err(|_| vfs::Error::IO)?
                    .map_err(|error| Self::io_error_to_vfs(&error))?;
                    Ok::<(usize, PathBuf, Metadata), vfs::Error>((idx, path, metadata))
                });
            }

            while let Some(task_result) = tasks.join_next().await {
                let (idx, path, metadata) = task_result.map_err(|_| vfs::Error::IO)??;
                let attr = Self::attr_from_metadata(&metadata);

                self.attr_cache.attrs.insert(path.clone(), Arc::new(CachedAttrEntry { metadata }));
                self.attr_cache.keys.insert(path.clone()).await;
                self.attr_cache.key_index.write().await.insert(path);
                attrs[idx] = Some(attr);
            }
        }

        self.maybe_compact_attr_keys().await;

        attrs.into_iter().map(|attr| attr.ok_or(vfs::Error::IO)).collect()
    }

    async fn read_ahead_copy_hit(
        &self,
        path: &Path,
        offset: u64,
        requested: usize,
        data: &mut nfs_mamont::Slice,
    ) -> Option<usize> {
        if requested == 0 {
            return Some(0);
        }

        let block_size = READ_AHEAD_BLOCK_SIZE as u64;
        let mut copied = 0usize;
        let mut current_offset = offset;

        while copied < requested {
            let block_index = current_offset / block_size;
            let block_offset = (current_offset % block_size) as usize;
            let key = ReadAheadKey { path: path.to_path_buf(), block_index };
            let block = match self.read_ahead_cache.blocks.get(&key) {
                Some(block) => block,
                None => break,
            };

            if block_offset >= block.len() {
                break;
            }

            let available = block.len() - block_offset;
            let to_copy = available.min(requested - copied);
            if to_copy == 0 {
                break;
            }

            let mut left = to_copy;
            let mut dst_pos = copied;
            let mut src_pos = block_offset;
            for chunk in data.iter_mut() {
                if left == 0 {
                    break;
                }
                if dst_pos >= chunk.len() {
                    dst_pos -= chunk.len();
                    continue;
                }

                let writable = (chunk.len() - dst_pos).min(left);
                chunk[dst_pos..dst_pos + writable]
                    .copy_from_slice(&block[src_pos..src_pos + writable]);
                left -= writable;
                src_pos += writable;
                dst_pos = 0;
            }

            copied += to_copy;
            current_offset = current_offset.saturating_add(to_copy as u64);
        }

        if copied == 0 {
            None
        } else {
            Some(copied)
        }
    }

    async fn update_read_sequence(&self, path: &Path, start: u64, end: u64) -> bool {
        let now = Instant::now();
        let mut sequence = self.read_ahead_cache.sequence.lock().await;
        let sequential = sequence
            .get(path)
            .map(|state| {
                state.next_offset == start
                    && now.saturating_duration_since(state.last_seen) <= READ_AHEAD_SEQUENCE_WINDOW
            })
            .unwrap_or(false);

        sequence.insert(path.to_path_buf(), ReadSequenceState { next_offset: end, last_seen: now });

        if sequence.len() > READ_AHEAD_CACHE_LIMIT * 8 {
            sequence.retain(|_, state| {
                now.saturating_duration_since(state.last_seen)
                    <= READ_AHEAD_SEQUENCE_WINDOW.saturating_mul(4)
            });
        }

        sequential
    }

    async fn schedule_read_ahead(
        &self,
        path: &Path,
        file: Arc<File>,
        block_index: u64,
        file_len: u64,
    ) {
        let block_start = block_index.saturating_mul(READ_AHEAD_BLOCK_SIZE as u64);
        if block_start >= file_len {
            return;
        }

        let prefetch_size = (file_len - block_start).min(READ_AHEAD_BLOCK_SIZE as u64) as usize;
        if prefetch_size == 0 {
            return;
        }

        let key = ReadAheadKey { path: path.to_path_buf(), block_index };
        if self.read_ahead_cache.blocks.get(&key).is_some() {
            return;
        }

        {
            let key_index = self.read_ahead_cache.key_index.read().await;
            let existing_for_file = key_index.iter().filter(|known| known.path == key.path).count();
            if existing_for_file >= READ_AHEAD_PER_FILE_LIMIT {
                return;
            }
        }

        {
            let mut inflight = self.read_ahead_cache.inflight.lock().await;
            if !inflight.insert(key.clone()) {
                return;
            }
        }

        let cache = self.read_ahead_cache.clone();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                let mut loaded = vec![0u8; prefetch_size];
                let mut total = 0usize;

                while total < prefetch_size {
                    let read = file.read_at(&mut loaded[total..], block_start + total as u64)?;
                    if read == 0 {
                        break;
                    }
                    total += read;
                }

                loaded.truncate(total);
                Ok::<Vec<u8>, std::io::Error>(loaded)
            })
            .await;

            if let Ok(Ok(loaded)) = result {
                if !loaded.is_empty() {
                    cache.blocks.insert(key.clone(), Arc::<[u8]>::from(loaded));
                    cache.key_index.write().await.insert(key.clone());
                    cache.maybe_compact_keys().await;
                }
            }

            cache.inflight.lock().await.remove(&key);
        });
    }

    async fn schedule_read_ahead_window(
        &self,
        path: &Path,
        file: Arc<File>,
        start_block_index: u64,
        file_len: u64,
    ) {
        for block in 0..READ_AHEAD_WINDOW_BLOCKS {
            let block_index = start_block_index.saturating_add(block as u64);
            let block_start = block_index.saturating_mul(READ_AHEAD_BLOCK_SIZE as u64);
            if block_start >= file_len {
                break;
            }

            self.schedule_read_ahead(path, file.clone(), block_index, file_len).await;
        }
    }

    async fn invalidate_read_ahead_path(&self, path: &Path) {
        let keys: Vec<ReadAheadKey> = {
            let index = self.read_ahead_cache.key_index.read().await;
            index
                .iter()
                .filter(|known| known.path == path || known.path.starts_with(path))
                .cloned()
                .collect()
        };

        for key in keys {
            self.read_ahead_cache.blocks.invalidate(&key);
            self.read_ahead_cache.key_index.write().await.remove(&key);
            self.read_ahead_cache.inflight.lock().await.remove(&key);
        }

        let mut sequence = self.read_ahead_cache.sequence.lock().await;
        sequence.retain(|known_path, _| !(known_path == path || known_path.starts_with(path)));
    }

    fn ensure_name_allowed(name: &file::Name) -> Result<(), vfs::Error> {
        match name.as_str() {
            "." => Err(vfs::Error::InvalidArgument),
            ".." => Err(vfs::Error::Exist),
            _ => Ok(()),
        }
    }

    fn io_error_to_vfs(error: &std::io::Error) -> vfs::Error {
        match error.kind() {
            ErrorKind::NotFound => vfs::Error::NoEntry,
            ErrorKind::PermissionDenied => vfs::Error::Access,
            ErrorKind::AlreadyExists => vfs::Error::Exist,
            ErrorKind::InvalidInput | ErrorKind::InvalidData => vfs::Error::InvalidArgument,
            ErrorKind::DirectoryNotEmpty => vfs::Error::NotEmpty,
            ErrorKind::IsADirectory => vfs::Error::IsDir,
            ErrorKind::NotADirectory => vfs::Error::NotDir,
            ErrorKind::WriteZero => vfs::Error::NoSpace,
            _ => vfs::Error::IO,
        }
    }

    fn time_from_unix(seconds: i64, nanos: i64) -> file::Time {
        file::Time {
            seconds: seconds.max(0).min(u32::MAX as i64) as u32,
            nanos: nanos.max(0).min(u32::MAX as i64) as u32,
        }
    }

    fn system_time_from_file_time(time: file::Time) -> SystemTime {
        UNIX_EPOCH + Duration::new(u64::from(time.seconds), time.nanos)
    }

    fn same_time(left: file::Time, right: file::Time) -> bool {
        left.seconds == right.seconds && left.nanos == right.nanos
    }

    fn attr_from_metadata(meta: &Metadata) -> file::Attr {
        let file_type = meta.file_type();
        let file_type = if file_type.is_dir() {
            file::Type::Directory
        } else if file_type.is_symlink() {
            file::Type::Symlink
        } else if file_type.is_file() {
            file::Type::Regular
        } else if file_type.is_block_device() {
            file::Type::BlockDevice
        } else if file_type.is_char_device() {
            file::Type::CharacterDevice
        } else if file_type.is_fifo() {
            file::Type::Fifo
        } else if file_type.is_socket() {
            file::Type::Socket
        } else {
            file::Type::Regular
        };

        file::Attr {
            file_type,
            mode: meta.mode(),
            nlink: meta.nlink() as u32,
            uid: meta.uid(),
            gid: meta.gid(),
            size: meta.size(),
            used: meta.blocks().saturating_mul(512),
            device: file::Device { major: 0, minor: 0 },
            fs_id: meta.dev(),
            file_id: meta.ino(),
            atime: Self::time_from_unix(meta.atime(), meta.atime_nsec()),
            mtime: Self::time_from_unix(meta.mtime(), meta.mtime_nsec()),
            ctime: Self::time_from_unix(meta.ctime(), meta.ctime_nsec()),
        }
    }

    fn wcc_attr_from_metadata(meta: &Metadata) -> file::WccAttr {
        file::WccAttr {
            size: meta.size(),
            mtime: Self::time_from_unix(meta.mtime(), meta.mtime_nsec()),
            ctime: Self::time_from_unix(meta.ctime(), meta.ctime_nsec()),
        }
    }

    fn wcc_attr_from_attr(attr: &file::Attr) -> file::WccAttr {
        file::WccAttr { size: attr.size, mtime: attr.mtime, ctime: attr.ctime }
    }

    fn metadata(path: &Path) -> Result<Metadata, vfs::Error> {
        std::fs::symlink_metadata(path).map_err(|error| Self::io_error_to_vfs(&error))
    }

    fn wcc_data(path: &Path, before: Option<file::WccAttr>) -> vfs::WccData {
        vfs::WccData {
            before,
            after: std::fs::symlink_metadata(path).ok().map(|meta| Self::attr_from_metadata(&meta)),
        }
    }

    fn validate_directory(attr: &file::Attr) -> Result<(), vfs::Error> {
        if matches!(attr.file_type, file::Type::Directory) {
            Ok(())
        } else {
            Err(vfs::Error::NotDir)
        }
    }

    fn validate_regular(attr: &file::Attr) -> Result<(), vfs::Error> {
        if matches!(attr.file_type, file::Type::Regular) {
            Ok(())
        } else {
            Err(vfs::Error::InvalidArgument)
        }
    }

    fn file_attr(path: &Path) -> Option<file::Attr> {
        std::fs::symlink_metadata(path).ok().map(|meta| Self::attr_from_metadata(&meta))
    }

    /// Stores an exclusive create verifier in the file's mtime (per RFC 1813 §3.3.8).
    fn store_exclusive_verifier(path: &Path, verifier: &[u8; NFS3_CREATEVERFSIZE]) {
        let sec = u32::from_be_bytes(verifier[0..4].try_into().unwrap());
        let nsec = u32::from_be_bytes(verifier[4..8].try_into().unwrap());
        let time = UNIX_EPOCH + Duration::new(u64::from(sec), nsec);
        let times = std::fs::FileTimes::new().set_modified(time);
        if let Ok(file) = std::fs::File::open(path) {
            let _ = file.set_times(times);
        }
    }

    /// Checks if an existing file's mtime matches the exclusive create verifier.
    fn check_exclusive_verifier(path: &Path, verifier: &[u8; NFS3_CREATEVERFSIZE]) -> bool {
        let Ok(meta) = std::fs::symlink_metadata(path) else { return false };
        let stored_sec = meta.mtime() as u32;
        let stored_nsec = meta.mtime_nsec() as u32;
        let expected_sec = u32::from_be_bytes(verifier[0..4].try_into().unwrap());
        let expected_nsec = u32::from_be_bytes(verifier[4..8].try_into().unwrap());
        stored_sec == expected_sec && stored_nsec == expected_nsec
    }

    fn apply_set_attr(path: &Path, new_attr: &set_attr::NewAttr) -> Result<(), vfs::Error> {
        if new_attr.uid.is_some() || new_attr.gid.is_some() {
            return Err(vfs::Error::InvalidArgument);
        }

        if let Some(mode) = new_attr.mode {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
                .map_err(|error| Self::io_error_to_vfs(&error))?;
        }

        if let Some(size) = new_attr.size {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(path)
                .map_err(|error| Self::io_error_to_vfs(&error))?;
            file.set_len(size).map_err(|error| Self::io_error_to_vfs(&error))?;
        }

        let needs_atime = !matches!(new_attr.atime, set_attr::SetTime::DontChange);
        let needs_mtime = !matches!(new_attr.mtime, set_attr::SetTime::DontChange);
        if needs_atime || needs_mtime {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .open(path)
                .map_err(|error| Self::io_error_to_vfs(&error))?;
            let meta = file.metadata().map_err(|error| Self::io_error_to_vfs(&error))?;
            let current_attr = Self::attr_from_metadata(&meta);
            let atime = match new_attr.atime {
                set_attr::SetTime::DontChange => {
                    Self::system_time_from_file_time(current_attr.atime)
                }
                set_attr::SetTime::ToServer => SystemTime::now(),
                set_attr::SetTime::ToClient(time) => Self::system_time_from_file_time(time),
            };
            let mtime = match new_attr.mtime {
                set_attr::SetTime::DontChange => {
                    Self::system_time_from_file_time(current_attr.mtime)
                }
                set_attr::SetTime::ToServer => SystemTime::now(),
                set_attr::SetTime::ToClient(time) => Self::system_time_from_file_time(time),
            };
            let times = std::fs::FileTimes::new().set_accessed(atime).set_modified(mtime);
            file.set_times(times).map_err(|error| Self::io_error_to_vfs(&error))?;
        }

        Ok(())
    }

    async fn directory_entries_for(
        &self,
        dir_path: &Path,
        verifier: read_dir::CookieVerifier,
    ) -> Result<Arc<[DirectoryEntrySnapshot]>, vfs::Error> {
        if let Some(cached) = self.dir_listing_cache.listings.get(dir_path) {
            if cached.verifier == verifier {
                return Ok(Arc::clone(&cached.entries));
            }
        }

        let entries = Self::load_directory_entries(dir_path).await?;
        let key = dir_path.to_path_buf();
        let snapshot = Arc::new(DirectoryListingSnapshot {
            verifier,
            entries: Arc::<[DirectoryEntrySnapshot]>::from(entries),
        });
        let result = Arc::clone(&snapshot.entries);
        self.dir_listing_cache.listings.insert(key, snapshot);
        Ok(result)
    }

    async fn load_directory_entries(
        dir_path: &Path,
    ) -> Result<Vec<DirectoryEntrySnapshot>, vfs::Error> {
        let dir_path = dir_path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let mut entries = Vec::new();
            let listing =
                std::fs::read_dir(dir_path).map_err(|error| Self::io_error_to_vfs(&error))?;

            for item in listing {
                let item = item.map_err(|error| Self::io_error_to_vfs(&error))?;
                let file_name = item.file_name();
                let name = file::Name::new(file_name.to_string_lossy().into_owned())
                    .map_err(|_| vfs::Error::InvalidArgument)?;
                let path = item.path();
                let file_id = item.ino();
                entries.push(DirectoryEntrySnapshot { name, path, file_id });
            }

            entries.sort_by(|left, right| left.name.as_str().cmp(right.name.as_str()));
            Ok::<Vec<DirectoryEntrySnapshot>, vfs::Error>(entries)
        })
        .await
        .map_err(|_| vfs::Error::IO)?
    }

    async fn child_path(
        &self,
        dir: &file::Handle,
        name: &file::Name,
    ) -> Result<(usize, PathBuf), vfs::Error> {
        let (export_id, dir_path) = self.path_for_handle_with_export(dir).await?;
        let mut child = dir_path;
        child.push(name.as_str());
        Ok((export_id, child))
    }

    async fn exported_root_path(&self, export_id: usize) -> Result<PathBuf, vfs::Error> {
        Ok(self.export_state(export_id)?.root_path.clone())
    }

    fn compose_handle(export_id: usize, inner_handle: file::Handle) -> file::Handle {
        let inner_id = u64::from_be_bytes(inner_handle.0);
        debug_assert!(inner_id != 0);
        debug_assert!(inner_id <= LOCAL_HANDLE_MASK);
        file::Handle((((export_id as u64) << EXPORT_ID_SHIFT) | inner_id).to_be_bytes())
    }

    fn split_handle(&self, handle: &file::Handle) -> Result<(usize, file::Handle), vfs::Error> {
        let raw = u64::from_be_bytes(handle.0);
        let inner_id = raw & LOCAL_HANDLE_MASK;
        if inner_id == 0 {
            return Err(vfs::Error::BadFileHandle);
        }

        let export_id = (raw >> EXPORT_ID_SHIFT) as usize;
        if export_id >= self.exports.len() {
            return Err(vfs::Error::BadFileHandle);
        }

        Ok((export_id, file::Handle(inner_id.to_be_bytes())))
    }

    fn export_state(&self, export_id: usize) -> Result<&ExportState, vfs::Error> {
        self.exports.get(export_id).ok_or(vfs::Error::BadFileHandle)
    }
}
