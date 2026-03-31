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
    root_path: PathBuf,
    handle_to_path: DashMap<Handle, Arc<RwLock<PathBuf>>>,
    path_to_handle: DashMap<PathBuf, Handle>,
    next_id: AtomicU64,
}

impl HandleMap {
    pub fn new(root: PathBuf) -> Self {
        let id_to_handle = DashMap::new();
        let handle_to_id = DashMap::new();
        id_to_handle.insert(file::Handle(ROOT.to_be_bytes()), Arc::new(RwLock::new(root.clone())));
        handle_to_id.insert(root.clone(), file::Handle(ROOT.to_be_bytes()));
        Self {
            root_path: root,
            handle_to_path: id_to_handle,
            path_to_handle: handle_to_id,
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
        Ok(entry.value().clone())
    }

    pub async fn handle_for_path(&self, path: &PathBuf) -> Result<Handle, vfs::Error> {
        let entry = self.path_to_handle.get(path).ok_or(vfs::Error::StaleFile)?;
        Ok(entry.value().clone())
    }

    pub async fn create_handle(&self, path: &PathBuf) -> Result<Handle, vfs::Error> {
        let relative = path
            .strip_prefix(&self.root_path)
            .map_err(|_| vfs::Error::BadFileHandle)?
            .to_path_buf();

        let handle = match self.path_to_handle.get(path) {
            Some(prev) => prev.clone(),
            None => {
                let id = self.next_id.fetch_add(1, Ordering::Relaxed);
                let handle = file::Handle(id.to_be_bytes());
                let arc = Arc::new(RwLock::new(relative));
                self.handle_to_path.insert(handle.clone(), arc);
                self.path_to_handle.insert(path.clone(), handle.clone());
                handle
            }
        };

        Ok(handle)
    }

    pub async fn remove_handle(
        &self,
        handle: &file::Handle,
        path_buf: &PathBuf,
    ) -> Result<(), vfs::Error> {
        // if handle == &self.root() {
        //     return Err(vfs::Error::InvalidArgument);
        // }

        let removed_path = self.handle_to_path.remove(handle);
        let removed_handle = self.path_to_handle.remove(path_buf);

        if removed_handle.is_none() || removed_path.is_none() {
            return Err(vfs::Error::StaleFile);
        }

        Ok(())
    }

    pub async fn rename_path(&self, from: &Path, to: &Path) -> Result<(), vfs::Error> {
        let from_relative = from
            .strip_prefix(&self.root_path)
            .map_err(|_| vfs::Error::BadFileHandle)?
            .to_path_buf();
        let to_relative =
            to.strip_prefix(&self.root_path).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();

        if from_relative.is_absolute() || to_relative.is_absolute() {
            return Err(vfs::Error::BadFileHandle);
        }

        let mut found = None;

        for shard in self.handle_to_path.iter() {
            let path = shard.read().await;
            if *path == from_relative {
                found = Some(shard.key().clone());
                break;
            }

            if found.is_some() {
                break;
            }
        }

        let handle = found.ok_or(vfs::Error::StaleFile)?;

        let removed_path = self.handle_to_path.remove(&handle);
        let removed_handle = self.path_to_handle.remove(&from_relative);

        if removed_handle.is_none() || removed_path.is_none() {
            return Err(vfs::Error::StaleFile);
        }

        self.handle_to_path.insert(handle, Arc::new(RwLock::new(to_relative)));

        Ok(())
    }

    pub fn to_full_path(&self, relative: &Path) -> PathBuf {
        if relative.as_os_str().is_empty() {
            self.root_path.clone()
        } else {
            self.root_path.join(relative)
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
