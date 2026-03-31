use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::RwLock;

use crate::vfs;
use crate::vfs::file;
use crate::vfs::file::Handle;

const ROOT: u64 = 0;

pub struct HandleMap {
    root: PathBuf,
    handle_to_path: DashMap<Handle, Arc<RwLock<PathBuf>>>,
    next_id: AtomicU64,
}

impl HandleMap {
    pub fn new(root: PathBuf) -> Self {
        Self { root, handle_to_path: DashMap::new(), next_id: AtomicU64::new(ROOT + 1) }
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

    pub async fn create_handle(&self, path: &Path) -> Result<Handle, vfs::Error> {
        let relative =
            path.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let handle = file::Handle(id.to_be_bytes());

        let arc = Arc::new(RwLock::new(relative));
        self.handle_to_path.insert(handle.clone(), arc);
        Ok(handle)
    }

    pub async fn remove_handle(&self, handle: &file::Handle) -> Result<(), vfs::Error> {
        if handle == &self.root() {
            return Err(vfs::Error::InvalidArgument);
        }

        let removed = self.handle_to_path.remove(handle);

        if removed.is_none() {
            return Err(vfs::Error::StaleFile);
        }

        Ok(())
    }

    pub async fn rename_path(&self, from: &Path, to: &Path) -> Result<(), vfs::Error> {
        let from_relative =
            from.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();
        let to_relative =
            to.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();

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

        let removed = self.handle_to_path.remove(&handle);
        if removed.is_none() {
            return Err(vfs::Error::StaleFile);
        }

        self.handle_to_path.insert(handle, Arc::new(RwLock::new(to_relative)));

        Ok(())
    }

    pub fn to_full_path(&self, relative: &Path) -> PathBuf {
        if relative.as_os_str().is_empty() {
            self.root.clone()
        } else {
            self.root.join(relative)
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
