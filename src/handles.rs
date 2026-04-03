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
    directory_to_children: DashMap<PathBuf, BTreeSet<Handle>>,
    next_id: AtomicU64,
}

impl HandleMap {
    pub fn new(root: PathBuf) -> Self {
        let root_handle = file::Handle(ROOT.to_be_bytes());
        let root_relative = PathBuf::new();
        let handle_to_path = DashMap::new();
        handle_to_path.insert(root_handle.clone(), Arc::new(RwLock::new(root_relative.clone())));
        let path_to_handle = DashMap::new();
        path_to_handle.insert(root_relative.clone(), root_handle.clone());

        let directory_to_children = DashMap::new();
        directory_to_children.insert(root_relative.clone(), BTreeSet::new());

        Self {
            root,
            handle_to_path,
            path_to_handle,
            directory_to_children,
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

        self.directory_to_children.entry(path.to_path_buf()).or_default();
        self.add_child_to_directory(path.parent().ok_or(vfs::Error::ServerFault)?, handle.clone());
        Ok(handle)
    }

    // not atomic by design; if lock needed, suggest it is taken before calling this function
    pub async fn remove_path(&self, path: &Path) -> Result<(), vfs::Error> {
        let (_, handle) = self.path_to_handle.remove(path).ok_or(vfs::Error::StaleFile)?;
        if let Some(parent) = path.parent() {
            self.remove_child_from_directory(parent, &handle);
        }
        if self.handle_to_path.remove(&handle).is_none() {
            return Err(vfs::Error::StaleFile);
        }
        self.directory_to_children.remove(path);

        Ok(())
    }

    pub async fn rename_path(
        &self,
        from: &Path,
        to: &Path,
        from_handle: Handle,
        to_handle: Option<Handle>,
    ) -> Result<(), vfs::Error> {
        let from_parent = from.parent().ok_or(vfs::Error::ServerFault)?;
        let to_parent = to.parent().ok_or(vfs::Error::ServerFault)?;

        if let Some(handle) = to_handle {
            self.remove_child_from_directory(to_parent, &handle);
            if self.path_to_handle.remove(to).is_none() {
                unreachable!("Path must exist, since we hold write lock");
            }
            if self.handle_to_path.remove(&handle).is_none() {
                unreachable!("Handle must exist, since we hold write lock");
            }
            self.directory_to_children.remove(to);
        }

        self.remove_child_from_directory(from_parent, &from_handle);
        self.add_child_to_directory(to_parent, from_handle.clone());
        if self.path_to_handle.remove(from).is_none() {
            unreachable!("Path must exist, since we hold write lock");
        }
        self.path_to_handle.insert(to.to_path_buf(), from_handle.clone());
        self.handle_to_path.alter(&from_handle, |_, _| Arc::new(RwLock::new(to.to_path_buf())));
        if let Some((_, children)) = self.directory_to_children.remove(from) {
            self.directory_to_children.insert(to.to_path_buf(), children);
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

    fn add_child_to_directory(&self, directory: &Path, handle: Handle) {
        match self.directory_to_children.get_mut(directory) {
            Some(mut children) => {
                children.insert(handle);
            }
            None => {
                let mut children = BTreeSet::new();
                children.insert(handle);
                self.directory_to_children.insert(directory.to_path_buf(), children);
            }
        }
    }

    fn remove_child_from_directory(&self, directory: &Path, handle: &Handle) {
        if let Some(mut children) = self.directory_to_children.get_mut(directory) {
            children.remove(handle);
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
