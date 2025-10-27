#[path = "../../examples/shadow_fs/fs/mod.rs"]
pub mod shadow_fs;

use std::path::PathBuf;

use nfs_mamont::vfs::{self, FileName, SetAttr, SetTime};
use shadow_fs::ShadowFS;
use tempfile::TempDir;

pub struct Fixture {
    pub tempdir: TempDir,
    pub fs: ShadowFS,
}

impl Fixture {
    pub fn new() -> Self {
        let tempdir = TempDir::new().expect("create temp dir");
        let fs = ShadowFS::new(tempdir.path().to_path_buf());
        Self { tempdir, fs }
    }

    pub fn root(&self) -> vfs::FileHandle {
        self.fs.root_handle()
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.tempdir.path().join(name)
    }

    pub fn write_file(&self, name: &str, data: &[u8]) {
        std::fs::write(self.path(name), data).expect("write fixture file");
    }

    pub fn create_dir(&self, name: &str) {
        std::fs::create_dir(self.path(name)).expect("create fixture dir");
    }
}

pub fn file_name(name: &str) -> FileName {
    FileName(name.to_owned())
}

pub fn empty_attr() -> SetAttr {
    SetAttr {
        mode: None,
        uid: None,
        gid: None,
        size: None,
        atime: SetTime::DontChange,
        mtime: SetTime::DontChange,
    }
}
