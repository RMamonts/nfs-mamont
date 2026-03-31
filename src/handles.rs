use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::RwLock;

use crate::vfs;
use crate::vfs::file;
use crate::vfs::file::Handle;

const ROOT: u64 = 1;

pub struct HandleMap {
    root: PathBuf,
    handle_to_path: DashMap<Handle, Arc<RwLock<PathBuf>>>,
    path_to_handle: DashMap<PathBuf, Handle>,
    key_to_paths: DashMap<Handle, Arc<RwLock<BTreeSet<PathBuf>>>>,
    next_id: AtomicU64,
}

impl HandleMap {
    pub fn new(root: PathBuf) -> Self {
        let root_handle = file::Handle(ROOT.to_be_bytes());
        let root_relative = PathBuf::new();
        let root_paths = Arc::new(RwLock::new(BTreeSet::from([root_relative.clone()])));
        let handle_to_path = DashMap::new();
        handle_to_path.insert(root_handle.clone(), Arc::new(RwLock::new(root_relative.clone())));
        let path_to_handle = DashMap::new();
        path_to_handle.insert(root_relative, root_handle.clone());
        let key_to_paths = DashMap::new();
        key_to_paths.insert(root_handle, root_paths);

        Self {
            root,
            handle_to_path,
            path_to_handle,
            key_to_paths,
            next_id: AtomicU64::new(ROOT + 1),
        }
    }

    pub fn root(&self) -> file::Handle {
        file::Handle(ROOT.to_be_bytes())
    }

    pub async fn path_for_handle(
        &self,
        handle: &file::Handle,
    ) -> Result<Arc<RwLock<PathBuf>>, vfs::Error> {
        let entry = self.handle_to_path.get(handle).ok_or(vfs::Error::StaleFile)?;
        let relative = entry.value().read().await.clone();
        Ok(Arc::new(RwLock::new(self.to_full_path(relative.as_path()))))
    }

    pub async fn handle_for_path(&self, path: &PathBuf) -> Result<Handle, vfs::Error> {
        let relative = self.relative_path(path.as_path())?;
        let entry = self.path_to_handle.get(&relative).ok_or(vfs::Error::StaleFile)?;
        Ok(entry.value().clone())
    }

    pub async fn create_handle(&self, path: &PathBuf) -> Result<Handle, vfs::Error> {
        let relative = self.relative_path(path.as_path())?;
        if let Some(prev) = self.path_to_handle.get(&relative) {
            return Ok(prev.value().clone());
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let handle = file::Handle(id.to_be_bytes());
        self.handle_to_path.insert(handle.clone(), Arc::new(RwLock::new(relative.clone())));
        self.path_to_handle.insert(relative.clone(), handle.clone());
        self.key_to_paths.insert(handle.clone(), Arc::new(RwLock::new(BTreeSet::from([relative]))));
        Ok(handle)
    }

    pub async fn remove_path(&self, path: &Path) -> Result<(), vfs::Error> {
        let relative = self.relative_path(path)?;

        let (_, handle) =
            self.path_to_handle.remove(relative.as_path()).ok_or(vfs::Error::StaleFile)?;
        let paths = self.key_to_paths.get(&handle).ok_or(vfs::Error::StaleFile)?.value().clone();

        let next_preferred = {
            let mut known_paths = paths.write().await;
            if !known_paths.remove(relative.as_path()) {
                return Err(vfs::Error::StaleFile);
            }
            known_paths.iter().next().cloned()
        };

        match next_preferred {
            Some(preferred) => {
                let lock =
                    self.handle_to_path.get(&handle).ok_or(vfs::Error::StaleFile)?.value().clone();
                let mut current_path = lock.write().await;
                if *current_path == relative {
                    *current_path = preferred;
                }
            }
            None => {
                self.key_to_paths.remove(&handle);
                self.handle_to_path.remove(&handle);
            }
        }

        Ok(())
    }

    pub async fn rename_path(&self, from: &Path, to: &Path) -> Result<(), vfs::Error> {
        let from_relative = self.relative_path(from)?;
        let to_relative = self.relative_path(to)?;

        let cached_source_paths = self.cached_paths_with_prefix(from_relative.as_path());
        if !cached_source_paths.iter().any(|path| path == &from_relative) {
            return Err(vfs::Error::StaleFile);
        }

        let overwritten_paths = self.cached_paths_with_prefix(to_relative.as_path());
        for old_path in overwritten_paths {
            if !cached_source_paths.iter().any(|source| source == &old_path) {
                let full_old_path = self.to_full_path(old_path.as_path());
                self.remove_path(full_old_path.as_path()).await?;
            }
        }

        for old_path in cached_source_paths {
            let (_, handle) =
                self.path_to_handle.remove(old_path.as_path()).ok_or(vfs::Error::StaleFile)?;
            let new_path = Self::replace_path_prefix(
                &old_path,
                from_relative.as_path(),
                to_relative.as_path(),
            )?;
            let paths =
                self.key_to_paths.get(&handle).ok_or(vfs::Error::StaleFile)?.value().clone();

            {
                let mut known_paths = paths.write().await;
                if !known_paths.remove(&old_path) {
                    return Err(vfs::Error::StaleFile);
                }
                known_paths.insert(new_path.clone());
            }

            self.path_to_handle.insert(new_path.clone(), handle.clone());

            let lock =
                self.handle_to_path.get(&handle).ok_or(vfs::Error::StaleFile)?.value().clone();
            let mut current_path = lock.write().await;
            if *current_path == old_path {
                *current_path = new_path;
            }
        }

        Ok(())
    }

    pub fn to_full_path(&self, relative: &Path) -> PathBuf {
        if relative.as_os_str().is_empty() {
            self.root.clone()
        } else {
            self.root.join(relative)
        }
    }

    fn cached_paths_with_prefix(&self, prefix: &Path) -> Vec<PathBuf> {
        self.path_to_handle
            .iter()
            .filter_map(|entry| {
                let path = entry.key();
                if path == prefix || path.starts_with(prefix) {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn replace_path_prefix(path: &Path, from: &Path, to: &Path) -> Result<PathBuf, vfs::Error> {
        let suffix = path.strip_prefix(from).map_err(|_| vfs::Error::ServerFault)?;
        if suffix.as_os_str().is_empty() {
            Ok(to.to_path_buf())
        } else {
            Ok(to.join(suffix))
        }
    }

    fn relative_path(&self, path: &Path) -> Result<PathBuf, vfs::Error> {
        path.strip_prefix(&self.root)
            .map(|relative| relative.to_path_buf())
            .map_err(|_| vfs::Error::BadFileHandle)
    }
}

pub fn ensure_name_allowed(name: &file::Name) -> Result<(), vfs::Error> {
    match name.as_str() {
        "." => Err(vfs::Error::InvalidArgument),
        ".." => Err(vfs::Error::Exist),
        _ => Ok(()),
    }
}
