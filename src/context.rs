#![allow(dead_code)]

use dashmap::DashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::vfs::file;
use crate::vfs::file::Handle;
use crate::{allocator::Impl, vfs};

pub struct HandleMap {
    root: PathBuf,
    handles: DashMap<Handle, PathBuf>,
    next_id: AtomicU64,
}

impl HandleMap {
    fn new(root: PathBuf) -> Self {
        let handles = DashMap::new();
        let root_handle = file::Handle(0u64.to_be_bytes());
        handles.insert(root_handle, root.clone());
        Self { root, handles, next_id: AtomicU64::new(1) }
    }

    fn root(&self) -> file::Handle {
        file::Handle(0u64.to_be_bytes())
    }

    pub fn path_for_handle(&self, handle: &file::Handle) -> Result<PathBuf, vfs::Error> {
        if handle == &self.root() {
            return Ok(self.root.clone());
        }
        Ok(self.handles.get(handle).ok_or(vfs::Error::StaleFile)?.value().clone())
    }

    pub fn rename_path(
        &mut self,
        handle: &file::Handle,
        from: &Path,
        to: &Path,
    ) -> Result<(), vfs::Error> {
        let from_relative =
            from.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();
        let to_relative =
            to.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();

        self.handles.get(handle).ok_or(vfs::Error::StaleFile)?;
        let (_, old_path) = self.handles.remove(handle).ok_or(vfs::Error::StaleFile)?;
        if from_relative != old_path {
            return Err(vfs::Error::InvalidArgument);
        }
        self.handles.insert(handle.clone(), to_relative.clone());
        Ok(())
    }

    fn to_full_path(&self, relative: &Path) -> PathBuf {
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
}
