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
    next_id: AtomicU64,
}

impl HandleMap {
    pub fn new(root: PathBuf) -> Self {
        let root_handle = file::Handle(ROOT.to_be_bytes());
        let root_relative = PathBuf::new();
        let handle_to_path = DashMap::new();
        handle_to_path.insert(root_handle.clone(), Arc::new(RwLock::new(root_relative.clone())));
        let path_to_handle = DashMap::new();
        path_to_handle.insert(root_relative, root_handle.clone());

        Self { root, handle_to_path, path_to_handle, next_id: AtomicU64::new(ROOT + 1) }
    }

    pub fn root(&self) -> file::Handle {
        file::Handle(ROOT.to_be_bytes())
    }

    pub async fn path_for_handle(
        &self,
        handle: &file::Handle,
    ) -> Result<Arc<RwLock<PathBuf>>, vfs::Error> {
        Ok(self.handle_to_path.get(handle).ok_or(vfs::Error::StaleFile)?.value().clone())
    }

    pub async fn handle_for_path(&self, path: &Path) -> Result<Handle, vfs::Error> {
        let entry = self.path_to_handle.get(path).ok_or(vfs::Error::StaleFile)?;
        Ok(entry.value().clone())
    }

    pub async fn create_handle(&self, path: &Path) -> Result<Handle, vfs::Error> {
        if let Some(prev) = self.path_to_handle.get(path) {
            return Ok(prev.value().clone());
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let handle = file::Handle(id.to_be_bytes());
        let path_lock = Arc::new(RwLock::new(path.to_path_buf()));
        self.handle_to_path.insert(handle.clone(), path_lock);
        self.path_to_handle.insert(path.to_path_buf(), handle.clone());
        Ok(handle)
    }

    // not atomic by design; if lock needed, suggest it is taken before calling this function
    pub async fn remove_path(&self, path: &Path) -> Result<(), vfs::Error> {
        let (_, handle) = self.path_to_handle.remove(path).ok_or(vfs::Error::StaleFile)?;
        if self.handle_to_path.remove(&handle).is_none() {
            return Err(vfs::Error::StaleFile);
        }

        Ok(())
    }

    pub async fn rename_path(
        &self,
        from: &Path,
        to: &Path,
        from_childs: Vec<(Handle, PathBuf)>,
        to_childs: Vec<(Handle, PathBuf)>,
    ) -> Result<(), vfs::Error> {
        for (handle, path) in to_childs {
            if self.path_to_handle.remove(&path).is_none() {
                unreachable!("Path must exist, since we hold write lock");
            }
            if self.handle_to_path.remove(&handle).is_none() {
                unreachable!("Handle must exist, since we hold write lock");
            }
        }

        for (handle, path) in from_childs {
            let new_path = Self::replace_path_prefix(&path, from, to)?;
            if self.path_to_handle.remove(&path).is_none() {
                unreachable!("Path must exist, since we hold write lock");
            }
            self.path_to_handle.insert(new_path.clone(), handle.clone());
            self.handle_to_path.alter(&handle, |_, _| Arc::new(RwLock::new(new_path)));
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

    //only with prefix but not path itself
    pub fn cached_paths_with_prefix(&self, prefix: &Path) -> Vec<(Handle, PathBuf)> {
        self.path_to_handle
            .iter()
            .filter_map(|entry| {
                let handle = entry.value();
                let path = entry.key();
                if path != prefix && path.starts_with(prefix) {
                    Some((handle.clone(), path.clone()))
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
}

pub fn ensure_name_allowed(name: &file::Name) -> Result<(), vfs::Error> {
    match name.as_str() {
        "." => Err(vfs::Error::InvalidArgument),
        ".." => Err(vfs::Error::Exist),
        _ => Ok(()),
    }
}
