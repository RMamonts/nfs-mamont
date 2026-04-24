use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

use nfs_mamont::vfs;
use nfs_mamont::vfs::file;

const FS_MAP_MUTATION_SHARDS: usize = 64;

/// Maps mirror paths to opaque VFS handles.
pub struct FsMap {
    root: PathBuf,
    next_id: AtomicU64,
    state: RwLock<FsMapState>,
    mutation_locks: Vec<Mutex<()>>,
}

struct FsMapState {
    id_to_key: HashMap<u64, ObjectKey>,
    key_to_id: HashMap<ObjectKey, u64>,
    key_to_paths: HashMap<ObjectKey, BTreeSet<PathBuf>>,
    relative_to_key: HashMap<PathBuf, ObjectKey>,
    relative_index: BTreeSet<PathBuf>,
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
            state: RwLock::new(FsMapState {
                id_to_key: HashMap::new(),
                key_to_id: HashMap::new(),
                key_to_paths: HashMap::new(),
                relative_to_key: HashMap::new(),
                relative_index: BTreeSet::new(),
            }),
            mutation_locks: (0..FS_MAP_MUTATION_SHARDS).map(|_| Mutex::new(())).collect(),
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

        let state = self.state.read().unwrap();
        let key = *state.id_to_key.get(&id).ok_or(vfs::Error::StaleFile)?;
        let paths = state.key_to_paths.get(&key).ok_or(vfs::Error::StaleFile)?;
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

        let shard = Self::mutation_shard_for_relative(&relative);
        let _guard = self.mutation_locks[shard].lock().unwrap();

        let key = ObjectKey { dev: attr.fs_id, ino: attr.file_id };
        {
            let mut state = self.state.write().unwrap();
            if let Some(id) = state.key_to_id.get(&key).copied() {
                Self::insert_relative_alias_locked(&mut state, relative, key);
                return Ok(Self::encode_handle(id));
            }

            let reserved = Self::reserve_next_id(&self.next_id);
            state.key_to_id.insert(key, reserved);
            state.id_to_key.insert(reserved, key);
            state.key_to_paths.entry(key).or_default();
            Self::insert_relative_alias_locked(&mut state, relative, key);

            return Ok(Self::encode_handle(reserved));
        }
    }

    pub async fn remove_path(&self, path: &Path) {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return;
        };
        let relative = relative.to_path_buf();

        let shard = Self::mutation_shard_for_relative(&relative);
        let _guard = self.mutation_locks[shard].lock().unwrap();

        let mut state = self.state.write().unwrap();
        let to_remove = state
            .relative_index
            .iter()
            .filter(|known_relative| *known_relative == &relative || known_relative.starts_with(&relative))
            .cloned()
            .collect::<Vec<_>>();

        if to_remove.is_empty() {
            return;
        }

        for known_relative in to_remove {
            let Some(key) = state.relative_to_key.remove(&known_relative) else {
                continue;
            };
            state.relative_index.remove(&known_relative);

            if let Some(paths) = state.key_to_paths.get_mut(&key) {
                paths.remove(&known_relative);
                if paths.is_empty() {
                    state.key_to_paths.remove(&key);
                    if let Some(id) = state.key_to_id.remove(&key) {
                        state.id_to_key.remove(&id);
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

        let from_shard = Self::mutation_shard_for_relative(&from_relative);
        let to_shard = Self::mutation_shard_for_relative(&to_relative);
        let (first, second) =
            if from_shard <= to_shard { (from_shard, to_shard) } else { (to_shard, from_shard) };

        let _first_guard = self.mutation_locks[first].lock().unwrap();
        let _second_guard = if second != first {
            Some(self.mutation_locks[second].lock().unwrap())
        } else {
            None
        };

        let mut state = self.state.write().unwrap();
        let to_rename = state
            .relative_index
            .iter()
            .filter(|known_relative| *known_relative == &from_relative || known_relative.starts_with(&from_relative))
            .cloned()
            .collect::<Vec<_>>();

        if to_rename.is_empty() {
            return Ok(());
        }

        for old_relative in to_rename {
            let Some(key) = state.relative_to_key.get(&old_relative).copied() else {
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

            state.relative_to_key.remove(&old_relative);
            state.relative_to_key.insert(new_relative.clone(), key);
            state.relative_index.remove(&old_relative);
            state.relative_index.insert(new_relative.clone());

            if let Some(paths) = state.key_to_paths.get_mut(&key) {
                paths.remove(&old_relative);
                paths.insert(new_relative);
            }
        }

        Ok(())
    }

    fn insert_relative_alias_locked(state: &mut FsMapState, relative: PathBuf, key: ObjectKey) {
        state.relative_to_key.insert(relative.clone(), key);
        state.relative_index.insert(relative.clone());
        state.key_to_paths.entry(key).or_default().insert(relative);
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
            if next_id.compare_exchange(current, next, Ordering::AcqRel, Ordering::Relaxed).is_ok()
            {
                return candidate;
            }
        }
    }

    fn mutation_shard_for_relative(relative: &Path) -> usize {
        let mut hasher = DefaultHasher::new();
        relative.hash(&mut hasher);
        (hasher.finish() as usize) % FS_MAP_MUTATION_SHARDS
    }
}
