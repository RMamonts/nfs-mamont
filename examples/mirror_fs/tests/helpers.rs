use std::fs as stdfs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use nfs_mamont::vfs;
use nfs_mamont::vfs::file;
use nfs_mamont::vfs::lookup;
use nfs_mamont::vfs::set_attr;
use nfs_mamont::Slice;

use crate::fs::MirrorFS;

pub fn expect_ok<T, E>(result: Result<T, E>, message: &str) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("{message}"),
    }
}

pub fn expect_err<T, E>(result: Result<T, E>, message: &str) -> E {
    match result {
        Ok(_) => panic!("{message}"),
        Err(error) => error,
    }
}

pub struct TestContext {
    tempdir: TempDir,
    pub fs: MirrorFS,
}

impl TestContext {
    pub fn new() -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let fs = MirrorFS::new(tempdir.path().to_path_buf());
        Self { tempdir, fs }
    }

    pub fn root_path(&self) -> &Path {
        self.tempdir.path()
    }

    pub async fn root_handle(&self) -> file::Handle {
        self.fs.root_handle().await
    }

    pub async fn lookup_handle(&self, parent: file::Handle, child_name: &str) -> file::Handle {
        expect_ok(
            lookup::Lookup::lookup(&self.fs, lookup::Args { parent, name: name(child_name) }).await,
            "lookup should succeed",
        )
        .file
    }
}

pub struct MultiExportTestContext {
    tempdirs: Vec<TempDir>,
    pub fs: MirrorFS,
}

impl MultiExportTestContext {
    pub fn new(export_count: usize) -> Self {
        let tempdirs = (0..export_count).map(|_| tempfile::tempdir().unwrap()).collect::<Vec<_>>();
        let fs = MirrorFS::new_many(
            tempdirs.iter().map(|tempdir| tempdir.path().to_path_buf()).collect(),
        );
        Self { tempdirs, fs }
    }

    pub fn root_path(&self, export_id: usize) -> &Path {
        self.tempdirs[export_id].path()
    }

    pub async fn root_handle(&self, export_id: usize) -> file::Handle {
        self.fs.root_handle_for_export(export_id).await
    }

    pub async fn lookup_handle(&self, parent: file::Handle, child_name: &str) -> file::Handle {
        expect_ok(
            lookup::Lookup::lookup(&self.fs, lookup::Args { parent, name: name(child_name) }).await,
            "lookup should succeed",
        )
        .file
    }
}

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
    set_attr::NewAttr {
        mode: None,
        uid: None,
        gid: None,
        size: None,
        atime: set_attr::SetTime::DontChange,
        mtime: set_attr::SetTime::DontChange,
    }
}

pub fn sized_attr(mode: Option<u32>, size: Option<u64>) -> set_attr::NewAttr {
    set_attr::NewAttr { mode, size, ..default_new_attr() }
}

pub async fn alloc_slice(len: usize) -> Slice {
    if len == 0 {
        return Slice::empty();
    }

    let len = NonZeroUsize::new(len).unwrap().get();
    Slice::new(vec![vec![0; len].into_boxed_slice()], 0..len, None)
}

pub async fn slice_from_bytes(bytes: &[u8]) -> Slice {
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

pub fn slice_to_vec(slice: &Slice) -> Vec<u8> {
    let mut data = Vec::new();
    for chunk in slice {
        data.extend_from_slice(chunk);
    }
    data
}

pub fn create_dir(root: &Path, relative: &str) -> PathBuf {
    let path = root.join(relative);
    stdfs::create_dir_all(&path).unwrap();
    path
}

pub fn write_file(root: &Path, relative: &str, data: &[u8]) -> PathBuf {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        stdfs::create_dir_all(parent).unwrap();
    }
    stdfs::write(&path, data).unwrap();
    path
}

pub fn create_symlink(root: &Path, target: &str, relative_link: &str) -> PathBuf {
    let link_path = root.join(relative_link);
    if let Some(parent) = link_path.parent() {
        stdfs::create_dir_all(parent).unwrap();
    }
    std::os::unix::fs::symlink(target, &link_path).unwrap();
    link_path
}

pub fn assert_wcc_present(wcc: &vfs::WccData) {
    assert!(wcc.before.is_some());
    assert!(wcc.after.is_some());
}
