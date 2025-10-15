use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::sync::RwLock;

use nfs_mamont::vfs::{
    AccessMask, AccessResult, AttrDigest, CommitResult, CookieVerifier, CreateMode, CreatedNode,
    DirectoryCookie, FileAttr, FileHandle, FileName, FileTime, FileType, FsInfo, FsStat,
    LinkResult, LookupResult, NfsError, PathConfig, ReadDirPlusResult, ReadDirResult, ReadResult,
    RemovalResult, RenameResult, SetAttr, SetAttrGuard, SetTime, SpecialNode, StableVerifier,
    SymlinkTarget, Vfs, VfsResult, WccData, WriteMode, WriteResult, MAX_FILE_HANDLE_LEN,
    MAX_NAME_LEN,
};

/// A tiny, in-memory VFS implementation
pub struct InMemoryVfs {
    // TODO: remove when will be used
    #[allow(unused)]
    state: RwLock<State>,
}

impl Default for InMemoryVfs {
    fn default() -> Self {
        Self { state: RwLock::new(State::new()) }
    }
}

impl InMemoryVfs {
    // TODO: remove when will be used
    #[allow(unused)]
    pub fn new() -> Self {
        Self::default()
    }

    // TODO: remove when will be used or remove
    #[allow(unused)]
    fn handle_to_path(handle: &FileHandle) -> VfsResult<String> {
        if handle.0.len() > MAX_FILE_HANDLE_LEN {
            return Err(NfsError::BadHandle);
        }
        String::from_utf8(handle.0.clone()).map_err(|_| NfsError::BadHandle)
    }

    // TODO: remove when will be used or remove
    #[allow(unused)]
    fn path_to_handle(path: &str) -> VfsResult<FileHandle> {
        if path.len() > MAX_FILE_HANDLE_LEN {
            return Err(NfsError::BadHandle);
        }
        Ok(FileHandle(path.as_bytes().to_vec()))
    }

    // TODO: remove when will be used or remove
    #[allow(unused)]
    fn validate_name(name: &FileName) -> VfsResult<()> {
        if name.0.is_empty() || name.0.len() > MAX_NAME_LEN || name.0.contains('/') {
            return Err(NfsError::NameTooLong);
        }
        Ok(())
    }

    // TODO: remove when will be used or remove
    #[allow(unused)]
    fn join(parent: &str, name: &FileName) -> String {
        if parent == "/" {
            format!("/{}", name.0)
        } else {
            format!("{}/{}", parent.trim_end_matches('/'), name.0)
        }
    }

    fn now() -> FileTime {
        let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        FileTime { seconds: duration.as_secs() as i64, nanos: duration.subsec_nanos() }
    }

    // TODO: remove when will be used or remove
    #[allow(unused)]
    fn digest(attr: &FileAttr) -> AttrDigest {
        AttrDigest { size: attr.size, mtime: attr.mtime, ctime: attr.ctime }
    }

    fn default_attr(file_type: FileType, fileid: u64) -> FileAttr {
        let now = Self::now();
        FileAttr {
            file_type,
            mode: match file_type {
                FileType::Directory => 0o755,
                _ => 0o644,
            },
            nlink: 1,
            uid: 0,
            gid: 0,
            size: 0,
            used: 0,
            device: None,
            fsid: 1,
            fileid,
            atime: now,
            mtime: now,
            ctime: now,
        }
    }

    // TODO: remove when will be used or remove
    #[allow(unused)]
    fn apply_attr(entry: &mut Entry, changes: &SetAttr) -> VfsResult<()> {
        let now = Self::now();
        if let Some(mode) = changes.mode {
            entry.attr.mode = mode;
        }
        if let Some(uid) = changes.uid {
            entry.attr.uid = uid;
        }
        if let Some(gid) = changes.gid {
            entry.attr.gid = gid;
        }
        if let Some(size) = changes.size {
            match &mut entry.kind {
                EntryKind::File { data } => {
                    let size_usize = size as usize;
                    if data.len() < size_usize {
                        data.resize(size_usize, 0);
                    } else {
                        data.truncate(size_usize);
                    }
                    entry.attr.size = size;
                    entry.attr.used = size;
                }
                _ => return Err(NfsError::Inval),
            }
        }
        match changes.atime {
            SetTime::DontChange => {}
            SetTime::ServerCurrent => entry.attr.atime = now,
            SetTime::ClientProvided(value) => entry.attr.atime = value,
        }
        match changes.mtime {
            SetTime::DontChange => {}
            SetTime::ServerCurrent => entry.attr.mtime = now,
            SetTime::ClientProvided(value) => entry.attr.mtime = value,
        }
        entry.attr.ctime = now;
        Ok(())
    }
}

struct State {
    nodes: HashMap<String, Entry>,
    next_fileid: u64,
    // TODO: remove when will be used or remove
    #[allow(unused)]
    stable_verifier: StableVerifier,
}

impl State {
    fn new() -> Self {
        let mut state = State {
            nodes: HashMap::new(),
            next_fileid: 1,
            stable_verifier: StableVerifier([0; 8]),
        };
        state.insert_root();
        state
    }

    fn insert_root(&mut self) {
        let id = self.next_id();
        let attr = InMemoryVfs::default_attr(FileType::Directory, id);
        let entry = Entry { attr, kind: EntryKind::Directory { children: BTreeMap::new() } };
        self.nodes.insert("/".into(), entry);
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_fileid;
        self.next_fileid += 1;
        id
    }
}

struct Entry {
    attr: FileAttr,
    kind: EntryKind,
}

enum EntryKind {
    // TODO: remove when will be used or remove
    #[allow(unused)]
    Directory { children: BTreeMap<String, String> },
    // TODO: remove when will be used or remove
    #[allow(unused)]
    File { data: Vec<u8> },
    // TODO: remove when will be used or remove
    #[allow(unused)]
    Symlink { target: String },
}

impl Entry {
    // TODO: remove when will be used or remove
    #[allow(unused)]
    fn as_directory(&self) -> Option<&BTreeMap<String, String>> {
        match &self.kind {
            EntryKind::Directory { children } => Some(children),
            _ => None,
        }
    }
}

#[async_trait]
impl Vfs for InMemoryVfs {
    async fn null(&self) -> VfsResult<()> {
        Ok(())
    }

    async fn get_attr(&self, _handle: &FileHandle) -> VfsResult<FileAttr> {
        unimplemented!()
    }

    async fn set_attr(
        &self,
        _handle: &FileHandle,
        _attr: SetAttr,
        _guard: SetAttrGuard,
    ) -> VfsResult<WccData> {
        unimplemented!()
    }

    async fn lookup(&self, _parent: &FileHandle, _name: &FileName) -> VfsResult<LookupResult> {
        unimplemented!()
    }

    async fn access(&self, _handle: &FileHandle, _mask: AccessMask) -> VfsResult<AccessResult> {
        unimplemented!()
    }

    async fn read_link(
        &self,
        _handle: &FileHandle,
    ) -> VfsResult<(SymlinkTarget, Option<FileAttr>)> {
        unimplemented!()
    }

    async fn read(&self, _handle: &FileHandle, _offset: u64, _count: u32) -> VfsResult<ReadResult> {
        unimplemented!()
    }

    async fn write(
        &self,
        _handle: &FileHandle,
        _offset: u64,
        _data: &[u8],
        _mode: WriteMode,
    ) -> VfsResult<WriteResult> {
        unimplemented!()
    }

    async fn create(
        &self,
        _parent: &FileHandle,
        _name: &FileName,
        _mode: CreateMode,
    ) -> VfsResult<CreatedNode> {
        unimplemented!()
    }

    async fn make_dir(
        &self,
        _parent: &FileHandle,
        _name: &FileName,
        _attr: SetAttr,
    ) -> VfsResult<CreatedNode> {
        unimplemented!()
    }

    async fn make_symlink(
        &self,
        _parent: &FileHandle,
        _name: &FileName,
        _target: &SymlinkTarget,
        _attr: SetAttr,
    ) -> VfsResult<CreatedNode> {
        unimplemented!()
    }

    async fn make_node(
        &self,
        _parent: &FileHandle,
        _name: &FileName,
        _node: SpecialNode,
    ) -> VfsResult<CreatedNode> {
        Err(NfsError::NotSupp)
    }

    async fn remove(&self, _parent: &FileHandle, _name: &FileName) -> VfsResult<RemovalResult> {
        unimplemented!()
    }

    async fn remove_dir(&self, _parent: &FileHandle, _name: &FileName) -> VfsResult<RemovalResult> {
        unimplemented!()
    }

    async fn rename(
        &self,
        _from_parent: &FileHandle,
        _from_name: &FileName,
        _to_parent: &FileHandle,
        _to_name: &FileName,
    ) -> VfsResult<RenameResult> {
        unimplemented!()
    }

    async fn link(
        &self,
        _source: &FileHandle,
        _new_parent: &FileHandle,
        _new_name: &FileName,
    ) -> VfsResult<LinkResult> {
        unimplemented!()
    }

    async fn read_dir(
        &self,
        _handle: &FileHandle,
        _cookie: DirectoryCookie,
        _verifier: CookieVerifier,
        _max_bytes: u32,
    ) -> VfsResult<ReadDirResult> {
        unimplemented!()
    }

    async fn read_dir_plus(
        &self,
        _handle: &FileHandle,
        _cookie: DirectoryCookie,
        _verifier: CookieVerifier,
        _max_bytes: u32,
        _max_handles: u32,
    ) -> VfsResult<ReadDirPlusResult> {
        unimplemented!()
    }

    async fn fs_stat(&self, _handle: &FileHandle) -> VfsResult<FsStat> {
        unimplemented!()
    }

    async fn fs_info(&self, _handle: &FileHandle) -> VfsResult<FsInfo> {
        unimplemented!()
    }

    async fn path_conf(&self, _handle: &FileHandle) -> VfsResult<PathConfig> {
        unimplemented!()
    }

    async fn commit(
        &self,
        _handle: &FileHandle,
        _offset: u64,
        _count: u32,
    ) -> VfsResult<CommitResult> {
        unimplemented!()
    }
}
