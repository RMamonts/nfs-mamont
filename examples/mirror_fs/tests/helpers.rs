use std::fs as stdfs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use nfs_mamont::allocator::{Allocator, Impl, Slice};
use nfs_mamont::vfs;
use nfs_mamont::vfs::file;
use nfs_mamont::vfs::lookup;
use nfs_mamont::vfs::set_attr;

use crate::fs::MirrorFS;

<<<<<<< HEAD
pub(crate) fn expect_ok<T, E>(result: Result<T, E>, message: &str) -> T {
=======
pub fn expect_ok<T, E>(result: Result<T, E>, message: &str) -> T {
>>>>>>> main
    match result {
        Ok(value) => value,
        Err(_) => panic!("{message}"),
    }
}

<<<<<<< HEAD
pub(crate) fn expect_err<T, E>(result: Result<T, E>, message: &str) -> E {
=======
pub fn expect_err<T, E>(result: Result<T, E>, message: &str) -> E {
>>>>>>> main
    match result {
        Ok(_) => panic!("{message}"),
        Err(error) => error,
    }
}

<<<<<<< HEAD
pub(crate) struct TestContext {
    tempdir: TempDir,
    pub(crate) fs: MirrorFS,
}

impl TestContext {
    pub(crate) fn new() -> Self {
=======
pub struct TestContext {
    tempdir: TempDir,
    pub fs: MirrorFS,
}

impl TestContext {
    pub fn new() -> Self {
>>>>>>> main
        let tempdir = tempfile::tempdir().unwrap();
        let fs = MirrorFS::new(tempdir.path().to_path_buf());
        Self { tempdir, fs }
    }

<<<<<<< HEAD
    pub(crate) fn root_path(&self) -> &Path {
        self.tempdir.path()
    }

    pub(crate) async fn root_handle(&self) -> file::Handle {
        self.fs.root_handle().await
    }

    pub(crate) async fn lookup_handle(
        &self,
        parent: file::Handle,
        child_name: &str,
    ) -> file::Handle {
=======
    pub fn root_path(&self) -> &Path {
        self.tempdir.path()
    }

    pub async fn root_handle(&self) -> file::Handle {
        self.fs.root_handle().await
    }

    pub async fn lookup_handle(&self, parent: file::Handle, child_name: &str) -> file::Handle {
>>>>>>> main
        expect_ok(
            lookup::Lookup::lookup(&self.fs, lookup::Args { parent, name: name(child_name) }).await,
            "lookup should succeed",
        )
        .file
    }
}

<<<<<<< HEAD
pub(crate) fn name(value: &str) -> file::Name {
    file::Name::new(value.to_owned()).unwrap()
}

pub(crate) fn file_path(value: &str) -> file::Path {
    file::Path::new(value.to_owned()).unwrap()
}

pub(crate) fn dir_op(dir: file::Handle, entry_name: &str) -> vfs::DirOpArgs {
    vfs::DirOpArgs { dir, name: name(entry_name) }
}

pub(crate) fn default_new_attr() -> set_attr::NewAttr {
=======
pub fn name(value: &str) -> file::Name {
    file::Name::new(value.to_owned()).unwrap()
}

pub fn file_path(value: &str) -> file::Path {
    file::Path::new(value.to_owned()).unwrap()
}

pub fn dir_op(dir: file::Handle, entry_name: &str) -> vfs::DirOpArgs {
    vfs::DirOpArgs { dir, name: name(entry_name) }
}

pub fn default_new_attr() -> set_attr::NewAttr {
>>>>>>> main
    set_attr::NewAttr {
        mode: None,
        uid: None,
        gid: None,
        size: None,
        atime: set_attr::SetTime::DontChange,
        mtime: set_attr::SetTime::DontChange,
    }
}

<<<<<<< HEAD
pub(crate) fn sized_attr(mode: Option<u32>, size: Option<u64>) -> set_attr::NewAttr {
    set_attr::NewAttr { mode, size, ..default_new_attr() }
}

pub(crate) async fn alloc_slice(len: usize) -> Slice {
=======
pub fn sized_attr(mode: Option<u32>, size: Option<u64>) -> set_attr::NewAttr {
    set_attr::NewAttr { mode, size, ..default_new_attr() }
}

pub async fn alloc_slice(len: usize) -> Slice {
>>>>>>> main
    if len == 0 {
        return Slice::empty();
    }

    let len = NonZeroUsize::new(len).unwrap();
    let mut allocator = Impl::new(len, NonZeroUsize::new(1).unwrap());

    allocator.allocate(len).await.expect("allocator must return a slice")
}

<<<<<<< HEAD
pub(crate) async fn slice_from_bytes(bytes: &[u8]) -> Slice {
=======
pub async fn slice_from_bytes(bytes: &[u8]) -> Slice {
>>>>>>> main
    let mut slice = alloc_slice(bytes.len()).await;
    let mut remaining = bytes;

    for chunk in slice.iter_mut() {
        if remaining.is_empty() {
            break;
        }

        let to_copy = chunk.len().min(remaining.len());
        chunk[..to_copy].copy_from_slice(&remaining[..to_copy]);
        remaining = &remaining[to_copy..];
    }

    slice
}

<<<<<<< HEAD
pub(crate) fn slice_to_vec(slice: &Slice) -> Vec<u8> {
=======
pub fn slice_to_vec(slice: &Slice) -> Vec<u8> {
>>>>>>> main
    let mut data = Vec::new();
    for chunk in slice {
        data.extend_from_slice(chunk);
    }
    data
}

<<<<<<< HEAD
pub(crate) fn create_dir(root: &Path, relative: &str) -> PathBuf {
=======
pub fn create_dir(root: &Path, relative: &str) -> PathBuf {
>>>>>>> main
    let path = root.join(relative);
    stdfs::create_dir_all(&path).unwrap();
    path
}

<<<<<<< HEAD
pub(crate) fn write_file(root: &Path, relative: &str, data: &[u8]) -> PathBuf {
=======
pub fn write_file(root: &Path, relative: &str, data: &[u8]) -> PathBuf {
>>>>>>> main
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        stdfs::create_dir_all(parent).unwrap();
    }
    stdfs::write(&path, data).unwrap();
    path
}

<<<<<<< HEAD
pub(crate) fn create_symlink(root: &Path, target: &str, relative_link: &str) -> PathBuf {
=======
pub fn create_symlink(root: &Path, target: &str, relative_link: &str) -> PathBuf {
>>>>>>> main
    let link_path = root.join(relative_link);
    if let Some(parent) = link_path.parent() {
        stdfs::create_dir_all(parent).unwrap();
    }
    std::os::unix::fs::symlink(target, &link_path).unwrap();
    link_path
}

<<<<<<< HEAD
pub(crate) fn assert_wcc_present(wcc: &vfs::WccData) {
=======
pub fn assert_wcc_present(wcc: &vfs::WccData) {
>>>>>>> main
    assert!(wcc.before.is_some());
    assert!(wcc.after.is_some());
}
