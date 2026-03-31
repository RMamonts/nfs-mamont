#![allow(dead_code)]

use dashmap::DashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::vfs::file;
use crate::vfs::file::Handle;
use crate::{allocator::Impl, vfs};

pub struct HandleMap {
    root: PathBuf,
    handles: DashMap<Handle, Arc<RwLock<PathBuf>>>,
    next_id: AtomicU64,
}

impl HandleMap {
    pub async fn new(root: PathBuf) -> Self {
        let handles = DashMap::new();
        let root_handle = file::Handle(0u64.to_be_bytes());

        handles.insert(root_handle, Arc::new(RwLock::new(root.clone())));

        Self { root, handles, next_id: AtomicU64::new(1) }
    }

    pub fn root(&self) -> file::Handle {
        file::Handle(0u64.to_be_bytes())
    }

    pub async fn path_for_handle(
        &self,
        handle: &file::Handle,
    ) -> Result<Arc<RwLock<PathBuf>>, vfs::Error> {
        let entry = self.handles.get(handle).ok_or(vfs::Error::StaleFile)?;
        Ok(entry.value().clone())
    }

    pub async fn create_handle(&self, relative: PathBuf) -> Result<Handle, vfs::Error> {
        if relative.is_absolute() {
            return Err(vfs::Error::InvalidArgument);
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let handle = file::Handle(id.to_be_bytes());

        let arc = Arc::new(RwLock::new(relative));
        self.handles.insert(handle.clone(), arc.clone());

        Ok(handle.clone())
    }

    pub async fn remove_handle(&self, handle: &file::Handle) -> Result<(), vfs::Error> {
        if handle == &self.root() {
            return Err(vfs::Error::InvalidArgument);
        }

        let removed = self.handles.remove(handle);

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

        let mut found = None;

        for shard in self.handles.iter() {
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

        let removed = self.handles.remove(&handle);
        if removed.is_none() {
            return Err(vfs::Error::StaleFile);
        }

        self.handles.insert(handle, Arc::new(RwLock::new(to_relative)));

        Ok(())
    }

    pub fn to_full_path(&self, relative: &Path) -> PathBuf {
        if relative.as_os_str().is_empty() {
            self.root.clone()
        } else {
            self.root.join(relative)
        }
    }

    fn map_io_error(error: std::io::Error) -> vfs::Error {
        match error.kind() {
            std::io::ErrorKind::NotFound => vfs::Error::NoEntry,
            std::io::ErrorKind::PermissionDenied => vfs::Error::Access,
            std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
                vfs::Error::InvalidArgument
            }
            _ => vfs::Error::IO,
        }
    }
}

pub struct ServerContext {
    read_allocator: Arc<Mutex<Impl>>,
    write_allocator: Arc<Mutex<Impl>>,
    backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
    handles: Arc<HandleMap>,
}

impl ServerContext {
    /// Builds context from allocator limits without exposing allocator implementation.
    pub fn new(
        backend: Arc<dyn vfs::Vfs + Send + Sync + 'static>,
        root: PathBuf,
        read_buffer_size: NonZeroUsize,
        read_buffer_count: NonZeroUsize,
        write_buffer_size: NonZeroUsize,
        write_buffer_count: NonZeroUsize,
    ) -> Self {
        let read_allocator = Arc::new(Mutex::new(Impl::new(read_buffer_size, read_buffer_count)));
        let write_allocator =
            Arc::new(Mutex::new(Impl::new(write_buffer_size, write_buffer_count)));
        let handles =
            Arc::new(HandleMap { handles: DashMap::new(), root, next_id: AtomicU64::new(1) });
        Self { read_allocator, write_allocator, backend, handles }
    }

    pub fn get_backend(&self) -> Arc<dyn vfs::Vfs + Send + Sync + 'static> {
        Arc::clone(&self.backend)
    }

    pub fn get_read_allocator(&self) -> Arc<Mutex<Impl>> {
        Arc::clone(&self.read_allocator)
    }

    pub fn get_write_allocator(&self) -> Arc<Mutex<Impl>> {
        Arc::clone(&self.write_allocator)
    }

    pub fn get_handle_map(&self) -> Arc<HandleMap> {
        Arc::clone(&self.handles)
    }
}
