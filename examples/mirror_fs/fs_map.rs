use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use whirlwind::ShardMap;

use nfs_mamont::vfs;
use nfs_mamont::vfs::file;

/// Maps mirror paths to opaque VFS handles.
pub struct FsMap {
    root: PathBuf,
    next_id: AtomicU64,
    id_to_key: ShardMap<u64, ObjectKey>,
    key_to_id: ShardMap<ObjectKey, u64>,
    key_to_paths: ShardMap<ObjectKey, Arc<RwLock<BTreeSet<PathBuf>>>>,
    relative_to_key: ShardMap<PathBuf, ObjectKey>,
    relative_index: RwLock<BTreeSet<PathBuf>>,
    mutation_lock: Mutex<()>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ObjectKey {
    dev: u64,
    ino: u64,
}

impl FsMap {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            next_id: AtomicU64::new(2),
            id_to_key: ShardMap::new(),
            key_to_id: ShardMap::new(),
            key_to_paths: ShardMap::new(),
            relative_to_key: ShardMap::new(),
            relative_index: RwLock::new(BTreeSet::new()),
            mutation_lock: Mutex::new(()),
        }
    }

    pub fn root_handle(&self) -> file::Handle {
        Self::encode_handle(1)
    }

    pub async fn path_candidates_for_handle(
        &self,
        handle: &file::Handle,
    ) -> Result<Vec<PathBuf>, vfs::Error> {
        let id = Self::decode_handle(handle)?;
        if id == 1 {
            return Ok(vec![self.root.clone()]);
        }

        let key = {
            let key_ref = self.id_to_key.get(&id).await.ok_or(vfs::Error::StaleFile)?;
            *key_ref.value()
        };

        let paths_lock = {
            let paths_ref = self.key_to_paths.get(&key).await.ok_or(vfs::Error::StaleFile)?;
            Arc::clone(paths_ref.value())
        };

        let paths = paths_lock.read().await;
        let full_paths = paths.iter().map(|relative| self.to_full_path(relative)).collect();
        Ok(full_paths)
    }

    pub async fn ensure_handle_for_attr(
        &self,
        path: &Path,
        attr: &file::Attr,
    ) -> Result<file::Handle, vfs::Error> {
        let relative =
            path.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();

        if relative.as_os_str().is_empty() {
            return Ok(self.root_handle());
        }

        let _guard = self.mutation_lock.lock().await;

        let key = ObjectKey { dev: attr.fs_id, ino: attr.file_id };
        if let Some(id) = {
            self.key_to_id.get(&key).await.map(|id_ref| *id_ref.value())
        } {
            self.insert_relative_alias(relative, key).await;
            return Ok(Self::encode_handle(id));
        }

        let reserved = Self::reserve_next_id(&self.next_id);
        self.key_to_id.insert(key, reserved).await;
        self.id_to_key.insert(reserved, key).await;
        self.key_to_paths.insert(key, Arc::new(RwLock::new(BTreeSet::new()))).await;
        self.insert_relative_alias(relative, key).await;

        Ok(Self::encode_handle(reserved))
    }

    pub async fn remove_path(&self, path: &Path) {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return;
        };
        let relative = relative.to_path_buf();

        let _guard = self.mutation_lock.lock().await;

        let to_remove = {
            let index = self.relative_index.read().await;
            index
                .iter()
                .filter(|known_relative| {
                    *known_relative == &relative || known_relative.starts_with(&relative)
                })
                .cloned()
                .collect::<Vec<_>>()
        };

        if to_remove.is_empty() {
            return;
        }

        for known_relative in to_remove {
            let Some(key) = self.relative_to_key.remove(&known_relative).await else {
                continue;
            };
            self.relative_index.write().await.remove(&known_relative);

            if let Some(paths_lock) = self.paths_lock_for_key(&key).await {

                let mut paths = paths_lock.write().await;
                paths.remove(&known_relative);
                let empty = paths.is_empty();
                drop(paths);

                if empty {
                    self.key_to_paths.remove(&key).await;
                    if let Some(id) = self.key_to_id.remove(&key).await {
                        self.id_to_key.remove(&id).await;
                    }
                }
            }
        }
    }

    pub async fn rename_path(&self, from: &Path, to: &Path) -> Result<(), vfs::Error> {
        let from_relative =
            from.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();
        let to_relative =
            to.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();

        let _guard = self.mutation_lock.lock().await;

        let to_rename = {
            let index = self.relative_index.read().await;
            index
                .iter()
                .filter(|known_relative| {
                    *known_relative == &from_relative || known_relative.starts_with(&from_relative)
                })
                .cloned()
                .collect::<Vec<_>>()
        };

        if to_rename.is_empty() {
            return Ok(());
        }

        for old_relative in to_rename {
            let Some(key) = ({
                self.relative_to_key
                    .get(&old_relative)
                    .await
                    .map(|key_ref| *key_ref.value())
            }) else {
                continue;
            };

            let suffix = old_relative
                .strip_prefix(&from_relative)
                .map_err(|_| vfs::Error::InvalidArgument)?
                .to_path_buf();
            let mut new_relative = to_relative.clone();
            if !suffix.as_os_str().is_empty() {
                new_relative.push(suffix);
            }

            self.relative_to_key.remove(&old_relative).await;
            self.relative_to_key.insert(new_relative.clone(), key).await;

            {
                let mut index = self.relative_index.write().await;
                index.remove(&old_relative);
                index.insert(new_relative.clone());
            }

            if let Some(paths_lock) = self.paths_lock_for_key(&key).await {

                let mut paths = paths_lock.write().await;
                paths.remove(&old_relative);
                paths.insert(new_relative);
            }
        }

        Ok(())
    }

    async fn insert_relative_alias(&self, relative: PathBuf, key: ObjectKey) {
        self.relative_to_key.insert(relative.clone(), key).await;
        self.relative_index.write().await.insert(relative.clone());

        if let Some(paths_lock) = self.paths_lock_for_key(&key).await {
            paths_lock.write().await.insert(relative);
        }
    }

    async fn paths_lock_for_key(&self, key: &ObjectKey) -> Option<Arc<RwLock<BTreeSet<PathBuf>>>> {
        self.key_to_paths.get(key).await.map(|paths_ref| Arc::clone(paths_ref.value()))
    }

    fn to_full_path(&self, relative: &Path) -> PathBuf {
        if relative.as_os_str().is_empty() {
            self.root.clone()
        } else {
            self.root.join(relative)
        }
    }

    fn encode_handle(id: u64) -> file::Handle {
        file::Handle(id.to_be_bytes())
    }

    fn decode_handle(handle: &file::Handle) -> Result<u64, vfs::Error> {
        let id = u64::from_be_bytes(handle.0);
        if id == 0 {
            Err(vfs::Error::BadFileHandle)
        } else {
            Ok(id)
        }
    }

    fn reserve_next_id(next_id: &AtomicU64) -> u64 {
        loop {
            let current = next_id.load(Ordering::Relaxed);
            let candidate = if current <= 1 { 2 } else { current };
            let next = candidate.wrapping_add(1).max(2);
            if next_id
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return candidate;
            }
        }
    }

}
