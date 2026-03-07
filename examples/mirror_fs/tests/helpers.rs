use std::fs as stdfs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tokio::sync::mpsc;

use nfs_mamont::allocator::Slice;
use nfs_mamont::vfs;
use nfs_mamont::vfs::file;
use nfs_mamont::vfs::lookup;
use nfs_mamont::vfs::set_attr;

use crate::fs::MirrorFS;

pub(crate) fn expect_ok<T, E>(result: Result<T, E>, message: &str) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("{message}"),
    }
}

pub(crate) fn expect_err<T, E>(result: Result<T, E>, message: &str) -> E {
    match result {
        Ok(_) => panic!("{message}"),
        Err(error) => error,
    }
}

pub(crate) struct TestContext {
    tempdir: TempDir,
    pub(crate) fs: MirrorFS,
}

impl TestContext {
    pub(crate) fn new() -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let fs = MirrorFS::new(tempdir.path().to_path_buf());
        Self { tempdir, fs }
    }

    pub(crate) fn root_path(&self) -> &Path {
        self.tempdir.path()
    }

    pub(crate) async fn root_handle(&self) -> file::Handle {
        self.fs.root_handle().await
    }

    pub(crate) async fn lookup_handle(&self, parent: file::Handle, child_name: &str) -> file::Handle {
        expect_ok(
            lookup::Lookup::lookup(&self.fs, lookup::Args { parent, name: name(child_name) }).await,
            "lookup should succeed",
        )
        .file
    }
}

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
    set_attr::NewAttr {
        mode: None,
        uid: None,
        gid: None,
        size: None,
        atime: set_attr::SetTime::DontChange,
        mtime: set_attr::SetTime::DontChange,
    }
}

pub(crate) fn sized_attr(mode: Option<u32>, size: Option<u64>) -> set_attr::NewAttr {
    set_attr::NewAttr { mode, size, ..default_new_attr() }
}

pub(crate) fn slice_from_bytes(bytes: &[u8]) -> Slice {
    let len = bytes.len();
    let buffer_len = NonZeroUsize::new(len.max(1)).unwrap();
    let mut buffer = vec![0u8; buffer_len.get()].into_boxed_slice();
    buffer[..len].copy_from_slice(bytes);
    let (sender, _receiver) = mpsc::unbounded_channel();
    Slice::new(vec![buffer], 0..len, sender)
}

pub(crate) fn slice_to_vec(slice: &Slice) -> Vec<u8> {
    let mut data = Vec::new();
    for chunk in slice {
        data.extend_from_slice(chunk);
    }
    data
}

pub(crate) fn create_dir(root: &Path, relative: &str) -> PathBuf {
    let path = root.join(relative);
    stdfs::create_dir_all(&path).unwrap();
    path
}

pub(crate) fn write_file(root: &Path, relative: &str, data: &[u8]) -> PathBuf {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        stdfs::create_dir_all(parent).unwrap();
    }
    stdfs::write(&path, data).unwrap();
    path
}

pub(crate) fn create_symlink(root: &Path, target: &str, relative_link: &str) -> PathBuf {
    let link_path = root.join(relative_link);
    if let Some(parent) = link_path.parent() {
        stdfs::create_dir_all(parent).unwrap();
    }
    std::os::unix::fs::symlink(target, &link_path).unwrap();
    link_path
}

pub(crate) fn assert_wcc_present(wcc: &vfs::WccData) {
    assert!(wcc.before.is_some());
    assert!(wcc.after.is_some());
}
