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
//! Conversion to absolute filesystem paths is performed via [`HandleMap::`to_full_path`]
//! using the configured root.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;

use crate::vfs;
use crate::vfs::file;
use crate::vfs::file::Handle;

const ROOT: u64 = 1;

/// A bidirectional mapping between NFS file handles and relative filesystem
/// paths, plus a directory → children index.
///
/// See module-level documentation for concurrency guarantees and expectations.
pub struct HandleMap {
    root: PathBuf,
    handle_to_path: DashMap<Handle, PathBuf>,
    path_to_handle: DashMap<PathBuf, Handle>,
    directory_to_children: DashMap<PathBuf, BTreeSet<Handle>>,
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

        let handle_to_path = DashMap::new();
        handle_to_path.insert(root_handle.clone(), root_relative.clone());

        let path_to_handle = DashMap::new();
        path_to_handle.insert(root_relative.clone(), root_handle);

        let directory_to_children = DashMap::new();
        directory_to_children.insert(root_relative, BTreeSet::new());

        Self {
            root,
            handle_to_path,
            path_to_handle,
            directory_to_children,
            next_id: AtomicU64::new(ROOT + 1),
        }
    }

    /// Returns the fixed handle representing the root directory.
    pub fn root(&self) -> file::Handle {
        file::Handle(ROOT.to_be_bytes())
    }

    /// Returns `true` if the given relative path represents the logical root.
    ///
    /// The root is encoded as an empty `PathBuf` rather than `"."` or `"/"`,
    /// which allows consistent relative path handling inside the map.
    pub fn is_root(path: &Path) -> bool {
        path.as_os_str().is_empty()
    }

    /// Resolves a handle into its associated absolute path.
    ///
    /// Returns `StaleFile` if the handle is unknown.
    pub fn path_for_handle(&self, handle: &file::Handle) -> Result<PathBuf, vfs::Error> {
        let relative =
            self.handle_to_path.get(handle).ok_or(vfs::Error::StaleFile)?.value().clone();
        Ok(self.to_full_path(relative.as_path()))
    }

    /// Resolves an absolute path into its associated handle.
    ///
    /// Returns `StaleFile` if the path is unknown.
    pub fn handle_for_path(&self, path: &Path) -> Result<Handle, vfs::Error> {
        let relative = self.to_relative_path(path).ok_or(vfs::Error::StaleFile)?;
        let entry = self.path_to_handle.get(&relative).ok_or(vfs::Error::StaleFile)?;
        Ok(entry.value().clone())
    }

    /// Creates a handle for the given path if it does not already exist.
    pub fn create_handle(&self, path: &Path) -> Result<Handle, vfs::Error> {
        let relative = self.to_relative_path(path).ok_or(vfs::Error::StaleFile)?;
        if let Some(prev) = self.path_to_handle.get(relative.as_path()) {
            return Ok(prev.value().clone());
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let handle = file::Handle(id.to_be_bytes());
        self.handle_to_path.insert(handle.clone(), relative.clone());
        self.path_to_handle.insert(relative, handle.clone());

        self.directory_to_children.entry(path.to_path_buf()).or_default();
        self.add_child_to_directory(path.parent().ok_or(vfs::Error::ServerFault)?, handle.clone());

        Ok(handle)
    }

    /// Removes a path and its associated handle.
    ///
    /// # Non-recursive
    /// Only the specific path is removed. Descendants are not touched.
    pub fn remove_path(&self, path: &Path) -> Result<(), vfs::Error> {
        let relative = self.to_relative_path(path).ok_or(vfs::Error::StaleFile)?;
        let (_, handle) =
            self.path_to_handle.remove(relative.as_path()).ok_or(vfs::Error::StaleFile)?;

        if let Some(parent) = relative.parent() {
            self.remove_child_from_directory(parent, &handle)?;
        }

        if self.handle_to_path.remove(&handle).is_none() {
            return Err(vfs::Error::StaleFile);
        }

        self.directory_to_children.remove(&relative);
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
        let to = self.to_relative_path(to).ok_or(vfs::Error::StaleFile)?;
        let from = self.to_relative_path(from).ok_or(vfs::Error::StaleFile)?;

        let from_parent = from.parent().ok_or(vfs::Error::ServerFault)?;
        let to_parent = to.parent().ok_or(vfs::Error::ServerFault)?;

        // If destination exists, remove it first.
        if let Some(handle) = to_handle {
            // ignore if entry has already been deleted
            self.remove_child_from_directory(to_parent, &handle)?;
            self.path_to_handle.remove(to.as_path());
            self.handle_to_path.remove(&handle);
            self.directory_to_children.remove(to.as_path());
        }

        self.remove_child_from_directory(from_parent, &from_handle)?;
        self.add_child_to_directory(to_parent, from_handle.clone())?;

        self.path_to_handle.remove(from.as_path());

        self.path_to_handle.insert(to.to_path_buf(), from_handle.clone());
        self.handle_to_path.alter(&from_handle, |_, _| to.to_path_buf());

        if let Some((_, children)) = self.directory_to_children.remove(from.as_path()) {
            self.directory_to_children.insert(to.to_path_buf(), children);
        }

        Ok(())
    }

    /// Converts a relative path into an absolute path under the configured root.
    fn to_full_path(&self, relative: &Path) -> PathBuf {
        if Self::is_root(relative) {
            self.root.clone()
        } else {
            self.root.join(relative)
        }
    }

    /// Converts an absolute filesystem path into a relative path under the
    /// configured root directory.
    ///
    /// If the provided path does not lie within the root, returns `None`.
    /// The root itself is represented as an empty `PathBuf`.
    fn to_relative_path(&self, full: &Path) -> Option<PathBuf> {
        full.strip_prefix(&self.root).ok().map(|path| {
            if Self::is_root(path) {
                PathBuf::new()
            } else {
                path.to_path_buf()
            }
        })
    }

    /// Adds a child handle to a directory entry.
    fn add_child_to_directory(&self, directory: &Path, handle: Handle) -> Result<(), vfs::Error> {
        let relative = self.to_relative_path(directory).ok_or(vfs::Error::StaleFile)?;
        match self.directory_to_children.get_mut(relative.as_path()) {
            Some(mut children) => {
                children.insert(handle);
            }
            None => {
                let mut children = BTreeSet::new();
                children.insert(handle);
                self.directory_to_children.insert(relative, children);
            }
        }
        Ok(())
    }

    /// Removes a child handle from a directory entry.
    fn remove_child_from_directory(
        &self,
        directory: &Path,
        handle: &Handle,
    ) -> Result<(), vfs::Error> {
        let relative = self.to_relative_path(directory).ok_or(vfs::Error::StaleFile)?;
        if let Some(mut children) = self.directory_to_children.get_mut(relative.as_path()) {
            children.remove(handle);
        }
        Ok(())
    }

    /// Returns all **direct children** of the given directory.
    ///
    /// The returned list contains handles of entries whose parent directory
    /// matches `path`. Only immediate children are included; this function does
    /// not traverse recursively or return descendants.
    ///
    /// If the directory has no children or is not tracked, an empty vector is
    /// returned.
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
