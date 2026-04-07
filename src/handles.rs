#![allow(dead_code)]
//! HandleMap provides a bidirectional mapping between NFS file handles and
//! relative filesystem paths, along with a directory → children index.
//!
//! # Concurrency model
//!
//! HandleMap is **not atomic** with respect to multi-table updates.
//! Operations such as creating, removing, or renaming a path update several
//! internal maps (`handle_to_path`, `path_to_handle`, `directory_to_children`),
//! but these updates are **not performed atomically as a single transaction**.
//!
//! # Non-recursive semantics
//!
//! Path removal and renaming are **non-recursive**.
//! Only the specific path entry is updated; descendants are *not* rewritten.
//!
//! This matches NFS semantics: child handles remain valid, and their paths will
//! be lazily corrected when accessed (e.g., via LOOKUP or READDIR).
//!
//! # Directory children
//!
//! `directory_to_children` tracks only **direct** children.
//! It is updated when handles are created, removed, or renamed, but does not
//! attempt to maintain a full recursive tree.
//!
//! # Full path resolution
//!
//! HandleMap stores only **relative** paths.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::vfs;
use crate::vfs::file;
use crate::vfs::file::{Handle, Name};

use dashmap::DashMap;
use tokio::sync::RwLock;

const ROOT: u64 = 1;

struct Descendant {
    handle: Handle,
    lock: Arc<RwLock<PathBuf>>,
}

struct Entry {
    path: Arc<RwLock<PathBuf>>,
    handle: Handle,
    descendants: DashMap<file::Name, Descendant>,
}

/// A bidirectional mapping between NFS file handles and relative filesystem
/// paths, plus a directory → children index.
///
/// See module-level documentation for concurrency guarantees and expectations.
pub struct HandleMap {
    root: PathBuf,
    handle_to_path: DashMap<Handle, Entry>,
    next_id: AtomicU64,
}

impl HandleMap {
    /// Creates a new HandleMap rooted at the given absolute path.
    ///
    /// The root directory is always represented by:
    /// - handle = fixed constant (`ROOT`)
    /// - relative path = empty `PathBuf`
    pub fn new(root: PathBuf) -> Self {
        let root_handle = file::Handle(ROOT.to_be_bytes());
        let root_relative = PathBuf::new();

        let entry = Entry {
            path: Arc::new(RwLock::new(root_relative)),
            handle: root_handle.clone(),
            descendants: DashMap::new(),
        };

        let handle_to_path = DashMap::new();
        handle_to_path.entry(root_handle).or_insert(entry);

        let next_id = AtomicU64::new(ROOT + 1);

        Self { root, handle_to_path, next_id }
    }

    /// Returns the fixed handle representing the root directory.
    pub fn root(&self) -> file::Handle {
        file::Handle(ROOT.to_be_bytes())
    }

    /// Resolves a handle into its associated relative path.
    ///
    /// Returns `StaleFile` if the handle is unknown.
    pub fn lock_for_handle(
        &self,
        handle: &file::Handle,
    ) -> Result<Arc<RwLock<PathBuf>>, vfs::Error> {
        Ok(self.handle_to_path.get(handle).ok_or(vfs::Error::StaleFile)?.path.clone())
    }

    pub fn lock_for_name(
        &self,
        parent: &Handle,
        file: &file::Name,
    ) -> Result<Arc<RwLock<PathBuf>>, vfs::Error> {
        let entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        let path = entry.descendants.get(file).ok_or(vfs::Error::StaleFile)?;
        Ok(path.lock.clone())
    }

    /// Creates a handle for the given path if it does not already exist.
    pub fn create_handle(
        &self,
        parent: &Handle,
        parent_path: &Path,
        file: &file::Name,
    ) -> Result<Handle, vfs::Error> {
        let entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let handle = file::Handle(id.to_be_bytes());
        let mut path = parent_path.to_owned();
        path.push(file.as_str());
        let lock = Arc::new(RwLock::new(path.clone()));
        entry
            .descendants
            .entry(file.clone())
            .or_insert(Descendant { handle: handle.clone(), lock: lock.clone() });
        self.handle_to_path.entry(handle.clone()).or_insert(Entry {
            path: lock,
            handle: handle.clone(),
            descendants: DashMap::new(),
        });
        Ok(handle)
    }

    /// Removes a path and its associated handle.
    ///
    /// # Non-recursive
    /// Only the specific path is removed. Descendants are not touched.
    pub fn remove_path(
        &self,
        parent: &Handle,
        name: &Name,
        prefix: &Handle,
    ) -> Result<(), vfs::Error> {
        self.handle_to_path.entry(parent.clone()).and_modify(|entry| {
            entry.descendants.remove(name);
        });

        self.remove_entry(prefix)?;

        Ok(())
    }
    fn remove_entry(&self, handle: &Handle) -> Result<(), vfs::Error> {
        let (_, entry) = self.handle_to_path.remove(handle).ok_or(vfs::Error::StaleFile)?;

        for child in entry.descendants.iter() {
            self.remove_entry(&child.handle)?;
        }
        Ok(())
    }

    pub fn collect_recursive_locks(
        &self,
        prefix: Handle,
    ) -> Result<Vec<Arc<RwLock<PathBuf>>>, vfs::Error> {
        let mut acc = Vec::new();
        self.collect_locks_with_acc(&mut acc, prefix)?;
        Ok(acc)
    }

    fn collect_locks_with_acc(
        &self,
        acc: &mut Vec<Arc<RwLock<PathBuf>>>,
        handle: Handle,
    ) -> Result<(), vfs::Error> {
        let entry = self.handle_to_path.get(&handle).ok_or(vfs::Error::StaleFile)?;
        acc.push(entry.path.clone());
        for child in entry.descendants.iter() {
            self.collect_locks_with_acc(acc, child.handle.clone())?;
        }
        Ok(())
    }

    /// Renames a path from `from` to `to`, updating all internal tables.
    ///
    /// # Non-recursive
    /// Only the specific path is updated. Descendants are not rewritten.
    pub fn rename_path(
        &self,
        from: &Path,
        to: &Path,
        from_handle: Handle,
        to_handle: Option<Handle>,
    ) -> Result<(), vfs::Error> {
        let from_parent = from.parent().ok_or(vfs::Error::ServerFault)?;
        let to_parent = to.parent().ok_or(vfs::Error::ServerFault)?;

        // If destination exists, remove it first.
        if let Some(handle) = to_handle {
            // ignore if entry has already been deleted
            self.remove_child_from_directory(to_parent, &handle);
            self.path_to_handle.remove(to);
            self.handle_to_path.remove(&handle);
            self.directory_to_children.remove(to);
        }

        self.remove_child_from_directory(from_parent, &from_handle);
        self.add_child_to_directory(to_parent, from_handle.clone());

        self.path_to_handle.remove(from);

        self.path_to_handle.insert(to.to_path_buf(), from_handle.clone());
        self.handle_to_path.alter(&from_handle, |_, _| to.to_path_buf());

        if let Some((_, children)) = self.directory_to_children.remove(from) {
            self.directory_to_children.insert(to.to_path_buf(), children);
        }

        Ok(())
    }

    /// Converts a relative path into an absolute path under the configured root.
    fn to_full_path(&self, relative: &Path) -> PathBuf {
        if relative.as_os_str().is_empty() {
            self.root.clone()
        } else {
            self.root.join(relative)
        }
    }

    /// Adds a child handle to a directory entry.
    fn add_child_to_directory(&self, directory: &Path, handle: Handle) {
        match self.directory_to_children.get_mut(directory) {
            Some(mut children) => {
                children.insert(handle);
            }
            None => {
                let mut children = BTreeSet::new();
                children.insert(handle);
                self.directory_to_children.insert(directory.to_path_buf(), children);
            }
        }
    }

    /// Removes a child handle from a directory entry.
    fn remove_child_from_directory(&self, directory: &Path, handle: &Handle) {
        if let Some(mut children) = self.directory_to_children.get_mut(directory) {
            children.remove(handle);
        }
    }

    fn get_children(&self, path: &Path) -> Vec<Handle> {
        match self.directory_to_children.get(path) {
            Some(children) => children.iter().cloned().collect(),
            None => Vec::new(),
        }
    }
}

/// Validates that a filename is allowed for NFS operations.
///
/// `"."` is rejected as `InvalidArgument`,
/// `".."` is rejected as `Exist`,
/// all other names are accepted.
pub fn ensure_name_allowed(name: &file::Name) -> Result<(), vfs::Error> {
    match name.as_str() {
        "." => Err(vfs::Error::InvalidArgument),
        ".." => Err(vfs::Error::Exist),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    /// Creates a fresh HandleMap with a dummy root.
    fn setup() -> HandleMap {
        HandleMap::new(PathBuf::from("/tmp"))
    }

    /// Asserts that all internal tables of HandleMap match the expected state.
    ///
    /// `expected_paths` is a list of (path, handle) pairs that must exist.
    /// This function checks:
    /// - path_to_handle contains exactly these entries
    /// - handle_to_path contains exactly these entries
    /// - directory_to_children contains correct children sets
    fn assert_state(map: &HandleMap, exp: &[(Handle, PathBuf)]) {
        let mut paths: Vec<(Handle, PathBuf)> =
            map.path_to_handle.iter().map(|e| (e.value().clone(), e.key().clone())).collect();
        paths.sort_by(|(_, h1), (_, h2)| h1.cmp(h2));

        let mut exp_sorted = exp.to_vec();
        exp_sorted.sort_by(|(_, h1), (_, h2)| h1.cmp(h2));

        assert_eq!(paths, exp_sorted);

        let mut handles: Vec<(Handle, PathBuf)> =
            map.handle_to_path.iter().map(|ent| (ent.key().clone(), ent.value().clone())).collect();
        handles.sort_by(|(_, h1), (_, h2)| h1.cmp(h2));

        assert_eq!(handles, exp_sorted);

        for (handle, path) in exp {
            if let Some(parent) = path.parent() {
                let children = map.get_children(parent);
                assert!(children.contains(handle));
            }
        }
    }

    #[test]
    fn test_multiple_insertions_and_state() {
        let map = setup();

        assert!(map.handle_for_path(Path::new("a")).is_err());
        assert!(map.path_for_handle(&Handle([9; 8])).is_err());

        let h_root = map.root();
        let h_a = map.create_handle(Path::new("a")).unwrap();
        let h_b = map.create_handle(Path::new("a/b")).unwrap();
        let h_c = map.create_handle(Path::new("a/c")).unwrap();
        let h_d = map.create_handle(Path::new("a/d")).unwrap();
        let h_e = map.create_handle(Path::new("a/d/e")).unwrap();

        assert_eq!(map.handle_for_path(Path::new("a")).unwrap(), h_a);
        assert_eq!(map.path_for_handle(&h_e).unwrap().as_path(), Path::new("a/d/e"));

        assert_state(
            &map,
            &[
                (h_root, PathBuf::new()),
                (h_a, PathBuf::from("a")),
                (h_b, PathBuf::from("a/b")),
                (h_c, PathBuf::from("a/c")),
                (h_d, PathBuf::from("a/d")),
                (h_e, PathBuf::from("a/d/e")),
            ],
        );
    }

    #[test]
    fn test_children_population() {
        let map = setup();

        assert!(map.handle_for_path(Path::new("x/1")).is_err());

        let h_x = map.create_handle(Path::new("x")).unwrap();
        let h1 = map.create_handle(Path::new("x/1")).unwrap();
        let h2 = map.create_handle(Path::new("x/2")).unwrap();
        let h3 = map.create_handle(Path::new("x/3")).unwrap();

        assert_eq!(map.handle_for_path(Path::new("x/2")).unwrap(), h2);
        assert_eq!(map.path_for_handle(&h3).unwrap().as_path(), Path::new("x/3"));

        let mut children = map.get_children(Path::new("x"));
        children.sort();
        let mut expected = vec![h1.clone(), h2.clone(), h3.clone()];
        expected.sort();

        assert_eq!(children, expected);

        assert_state(
            &map,
            &[
                (map.root(), PathBuf::new()),
                (h_x, PathBuf::from("x")),
                (h1, PathBuf::from("x/1")),
                (h2, PathBuf::from("x/2")),
                (h3, PathBuf::from("x/3")),
            ],
        );
    }

    #[test]
    fn test_existing_files_and_state() {
        let map = setup();

        assert!(map.handle_for_path(Path::new("dir/a")).is_err());

        let h_dir = map.create_handle(Path::new("dir")).unwrap();
        let h1 = map.create_handle(Path::new("dir/a")).unwrap();
        let h2 = map.create_handle(Path::new("dir/b")).unwrap();

        assert_eq!(map.handle_for_path(Path::new("dir/a")).unwrap(), h1);
        assert_eq!(map.path_for_handle(&h2).unwrap().as_path(), Path::new("dir/b"));

        let h1b = map.create_handle(Path::new("dir/a")).unwrap();
        assert_eq!(h1, h1b);

        assert_state(
            &map,
            &[
                (map.root(), PathBuf::new()),
                (h_dir, PathBuf::from("dir")),
                (h1, PathBuf::from("dir/a")),
                (h2, PathBuf::from("dir/b")),
            ],
        );
    }

    #[test]
    fn test_remove_updates_all_tables() {
        let map = setup();

        let h_p = map.create_handle(Path::new("p")).unwrap();
        let h1 = map.create_handle(Path::new("p/a")).unwrap();
        let h2 = map.create_handle(Path::new("p/b")).unwrap();

        assert_eq!(map.handle_for_path(Path::new("p/a")).unwrap(), h1);

        map.remove_path(Path::new("p/a")).unwrap();

        assert!(map.handle_for_path(Path::new("p/a")).is_err());
        assert!(map.path_for_handle(&h1).is_err());

        assert_state(
            &map,
            &[(map.root(), PathBuf::new()), (h_p, PathBuf::from("p")), (h2, PathBuf::from("p/b"))],
        );
    }

    #[test]
    fn test_rename_updates_all_tables() {
        let map = setup();

        let h_p = map.create_handle(Path::new("p")).unwrap();
        let h1 = map.create_handle(Path::new("p/a")).unwrap();

        assert_eq!(map.handle_for_path(Path::new("p/a")).unwrap(), h1.clone());

        map.rename_path(Path::new("p/a"), Path::new("p/z"), h1.clone(), None).unwrap();

        assert!(map.handle_for_path(Path::new("p/a")).is_err());
        assert_eq!(map.handle_for_path(Path::new("p/z")).unwrap(), h1);

        assert_state(
            &map,
            &[(map.root(), PathBuf::new()), (h_p, PathBuf::from("p")), (h1, PathBuf::from("p/z"))],
        );
    }
}
