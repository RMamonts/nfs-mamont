use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::mapref::entry::Entry;
use dashmap::DashMap;

use nfs_mamont::vfs;
use nfs_mamont::vfs::file;

/// Maps mirror paths to opaque VFS handles.
#[derive(Debug)]
pub struct FsMap {
    root: PathBuf,
    next_id: AtomicU64,
    id_to_key: DashMap<u64, ObjectKey>,
    key_to_id: DashMap<ObjectKey, u64>,
    key_to_paths: DashMap<ObjectKey, BTreeSet<PathBuf>>,
    relative_to_key: DashMap<PathBuf, ObjectKey>,
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
            id_to_key: DashMap::new(),
            key_to_id: DashMap::new(),
            key_to_paths: DashMap::new(),
            relative_to_key: DashMap::new(),
        }
    }

    pub fn root_handle(&self) -> file::Handle {
        Self::encode_handle(1)
    }

    pub fn path_candidates_for_handle(
        &self,
        handle: &file::Handle,
    ) -> Result<Vec<PathBuf>, vfs::Error> {
        let id = Self::decode_handle(handle)?;
        if id == 1 {
            return Ok(vec![self.root.clone()]);
        }

        let key = *self.id_to_key.get(&id).ok_or(vfs::Error::StaleFile)?;
        let paths = self.key_to_paths.get(&key).ok_or(vfs::Error::StaleFile)?;
        let full_paths = paths.iter().map(|relative| self.to_full_path(relative)).collect();
        Ok(full_paths)
    }

    pub fn ensure_handle_for_attr(
        &self,
        path: &Path,
        attr: &file::Attr,
    ) -> Result<file::Handle, vfs::Error> {
        let relative =
            path.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();

        if relative.as_os_str().is_empty() {
            return Ok(self.root_handle());
        }

        let key = ObjectKey { dev: attr.fs_id, ino: attr.file_id };
        if let Some(id) = self.key_to_id.get(&key).map(|entry| *entry.value()) {
            self.key_to_paths.entry(key).or_insert_with(BTreeSet::new).insert(relative.clone());
            self.relative_to_key.insert(relative, key);
            return Ok(Self::encode_handle(id));
        }

        let reserved = Self::reserve_next_id(&self.next_id);
        let id = match self.key_to_id.entry(key) {
            Entry::Occupied(existing) => *existing.get(),
            Entry::Vacant(vacant) => {
                vacant.insert(reserved);
                self.id_to_key.insert(reserved, key);
                reserved
            }
        };

        self.key_to_paths.entry(key).or_insert_with(BTreeSet::new).insert(relative.clone());
        self.relative_to_key.insert(relative, key);

        if id != reserved {
            self.id_to_key.remove(&reserved);
        }

        Ok(Self::encode_handle(id))
    }

    pub fn remove_path(&self, path: &Path) {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return;
        };
        let relative = relative.to_path_buf();

        let to_remove = self
            .relative_to_key
            .iter()
            .filter(|entry| {
                entry.key() == &relative || entry.key().starts_with(&relative)
            })
            .map(|entry| entry.key().clone())
            .collect::<Vec<_>>();

        for known_relative in to_remove {
            let Some(key) = self.relative_to_key.remove(&known_relative) else {
                continue;
            };
            let key = key.1;
            if let Some(mut paths) = self.key_to_paths.get_mut(&key) {
                paths.value_mut().remove(&known_relative);
                if paths.value().is_empty() {
                    drop(paths);
                    self.key_to_paths.remove(&key);
                    if let Some(id) = self.key_to_id.remove(&key).map(|(_, id)| id) {
                        self.id_to_key.remove(&id);
                    }
                }
            }
        }
    }

    pub fn rename_path(&self, from: &Path, to: &Path) -> Result<(), vfs::Error> {
        let from_relative =
            from.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();
        let to_relative =
            to.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();

        let updates = self
            .relative_to_key
            .iter()
            .filter_map(|entry| {
                let known_relative = entry.key();
                let key = *entry.value();
                if known_relative == &from_relative || known_relative.starts_with(&from_relative) {
                    let suffix = known_relative.strip_prefix(&from_relative).ok()?.to_path_buf();
                    let mut replacement = to_relative.clone();
                    if !suffix.as_os_str().is_empty() {
                        replacement.push(suffix);
                    }
                    Some((known_relative.clone(), key, replacement))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for (old_relative, key, new_relative) in updates {
            self.relative_to_key.remove(&old_relative);
            self.relative_to_key.insert(new_relative.clone(), key);

            if let Some(mut paths) = self.key_to_paths.get_mut(&key) {
                let values = paths.value_mut();
                values.remove(&old_relative);
                values.insert(new_relative);
            }
        }

        Ok(())
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
