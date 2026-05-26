use nfs_mamont::vfs::file;

pub const FILE_ATTR: file::Attr = file::Attr {
    file_type: file::Type::Regular,
    mode: 0o644,
    nlink: 1,
    uid: 1000,
    gid: 1000,
    size: 1073741824,
    used: 1073741824,
    device: file::Device { major: 0, minor: 0 },
    fs_id: 42,
    file_id: 1,
    atime: file::Time { seconds: 1000000, nanos: 0 },
    mtime: file::Time { seconds: 1000000, nanos: 0 },
    ctime: file::Time { seconds: 1000000, nanos: 0 },
};

pub const DIR_ATTR: file::Attr = file::Attr {
    file_type: file::Type::Directory,
    mode: 0o755,
    nlink: 2,
    uid: 1000,
    gid: 1000,
    size: 4096,
    used: 4096,
    device: file::Device { major: 0, minor: 0 },
    fs_id: 42,
    file_id: 2,
    atime: file::Time { seconds: 1000000, nanos: 0 },
    mtime: file::Time { seconds: 1000000, nanos: 0 },
    ctime: file::Time { seconds: 1000000, nanos: 0 },
};

#[derive(Clone)]
pub struct MockVfsConfig {
    pub file_size: u64,
    pub dir_entry_count: usize,
    pub default_attr: file::Attr,
    pub dir_attr: file::Attr,
}

impl Default for MockVfsConfig {
    fn default() -> Self {
        Self {
            file_size: 1073741824,
            dir_entry_count: 128,
            default_attr: FILE_ATTR,
            dir_attr: DIR_ATTR,
        }
    }
}
