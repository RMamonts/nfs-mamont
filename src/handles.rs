#![allow(dead_code)]
//! HandleMap stores a tree of handle-indexed entries.
//!
//! The map is intentionally split into structure and locking concerns:
//! - the internal methods do not acquire `RwLock` guards;
//! - callers are expected to take any required locks before they call mutating
//!   operations;
//! - HandleMap only rewires handles, path locks, and parent → child links.
//!
//! This keeps the data model simple enough for external synchronization while
//! still providing cheap handle lookups and parent/child navigation.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::RwLock;

use crate::vfs;
use crate::vfs::file;
use crate::vfs::file::Handle;

const ROOT: u64 = 1;

#[derive(Clone)]
struct Dasc {
    handle: Handle,
    lock: Arc<RwLock<PathBuf>>,
}

struct Entry {
    path: Arc<RwLock<PathBuf>>,
    handle: Handle,
    children: DashMap<file::Name, Dasc>,
}

impl Entry {
    fn new(handle: Handle, path: Arc<RwLock<PathBuf>>) -> Self {
        Self { path, handle, children: DashMap::new() }
    }
}

/// A bidirectional mapping between NFS file handles and relative filesystem
/// paths, plus a directory → children index.
///
/// See module-level documentation for concurrency guarantees and expectations.
pub struct HandleMap {
    root: PathBuf,
    map: Arc<DashMap<Handle, Entry>>,
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
        let root_lock = Arc::new(RwLock::new(root_relative.clone()));

        let map = DashMap::new();
        map.insert(root_handle.clone(), Entry::new(root_handle.clone(), root_lock));

        Self { root, map: Arc::new(map), next_id: AtomicU64::new(ROOT + 1) }
    }

    /// Returns the fixed handle representing the root directory.
    pub fn root(&self) -> file::Handle {
        file::Handle(ROOT.to_be_bytes())
    }

    /// Returns the `RwLock` holding the relative path for a handle.
    pub fn path_for_handle(
        &self,
        handle: &file::Handle,
    ) -> Result<Arc<RwLock<PathBuf>>, vfs::Error> {
        Ok(self.map.get(handle).ok_or(vfs::Error::StaleFile)?.value().path.clone())
    }

    /// Returns the direct child lock for a parent directory and entry name.
    pub fn path_for_child(
        &self,
        parent: &file::Handle,
        name: &file::Name,
    ) -> Result<Arc<RwLock<PathBuf>>, vfs::Error> {
        let parent_entry = self.map.get(parent).ok_or(vfs::Error::StaleFile)?;
        let child = parent_entry.children.get(name).ok_or(vfs::Error::StaleFile)?;
        Ok(child.value().lock.clone())
    }

    /// Returns the direct child handle for a parent handle and entry name.
    pub fn handle_for_child(
        &self,
        parent: &file::Handle,
        name: &file::Name,
    ) -> Result<Handle, vfs::Error> {
        let parent_entry = self.map.get(parent).ok_or(vfs::Error::StaleFile)?;
        let child = parent_entry.children.get(name).ok_or(vfs::Error::StaleFile)?;
        Ok(child.value().handle.clone())
    }

    /// Returns the direct child handle or creates a new entry for it.
    ///
    /// The provided path lock is stored in the new entry as-is.
    pub fn ensure_child_handle(
        &self,
        parent_path: &Path,
        parent: &Handle,
        name: &file::Name,
    ) -> Result<Handle, vfs::Error> {
        if let Ok(existing) = self.handle_for_child(parent, name) {
            return Ok(existing);
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let handle = file::Handle(id.to_be_bytes());
        let mut new_path = parent_path.to_owned();
        new_path.push(name.as_str());
        let lock = Arc::new(RwLock::new(new_path.to_path_buf()));

        let entry = Entry::new(handle.clone(), lock.clone());

        self.map.insert(handle.clone(), entry);

        let parent_entry = self.map.get(parent).ok_or(vfs::Error::StaleFile)?;
        parent_entry.children.insert(name.clone(), Dasc { handle: handle.clone(), lock });

        Ok(handle)
    }

    /// Removes a direct child entry from a parent directory.
    pub fn remove_child(&self, parent: &file::Handle, name: &file::Name) -> Result<(), vfs::Error> {
        let parent_entry = self.map.get(parent).ok_or(vfs::Error::StaleFile)?;
        let child = parent_entry.children.remove(name).ok_or(vfs::Error::StaleFile)?.1;

        if self.map.remove(&child.handle).is_none() {
            return Err(vfs::Error::StaleFile);
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

    fn remove_child_recursive(&self, parent: &Handle, name: &file::Name) -> Result<(), vfs::Error> {
        let parent_entry = self.map.get(parent).ok_or(vfs::Error::StaleFile)?;
        let (_, entry) = parent_entry.children.remove(name).ok_or(vfs::Error::StaleFile)?;

        self.remove_entry_recursive(&entry.handle)
    }

    fn remove_entry_recursive(&self, handle: &Handle) -> Result<(), vfs::Error> {
        let entry = self.map.get(handle).ok_or(vfs::Error::StaleFile)?;

        for child in entry.children.iter() {
            self.remove_entry_recursive(&child.handle)?;
        }

        self.map.remove(handle).ok_or(vfs::Error::StaleFile)?;
        Ok(())
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

    fn child_name(name: &str) -> file::Name {
        file::Name::new(name.to_string()).unwrap()
    }

    fn child_lock(path: &str) -> Arc<RwLock<PathBuf>> {
        Arc::new(RwLock::new(PathBuf::from(path)))
    }

    /// Asserts that the tree stored in HandleMap matches the expected state.
    fn assert_state(map: &HandleMap, exp: &[(Handle, PathBuf)]) {
        let mut actual: Vec<(Handle, PathBuf)> = map
            .map
            .iter()
            .map(|entry| {
                let path = entry.value().path.try_read().unwrap().clone();
                (entry.key().clone(), path)
            })
            .collect();
        actual.sort_by(|(_, left), (_, right)| left.cmp(right));

        let mut expected = exp.to_vec();
        expected.sort_by(|(_, left), (_, right)| left.cmp(right));

        assert_eq!(actual, expected);

        for (handle, path) in exp {
            let Some(entry) = map.map.get(handle) else {
                panic!("missing handle {handle:?}");
            };
            let mut children =
                entry.children.iter().map(|child| child.value().handle.clone()).collect::<Vec<_>>();
            children.sort();

            let mut expected_children = exp
                .iter()
                .filter_map(|(child_handle, child_path)| {
                    child_path
                        .parent()
                        .filter(|parent| *parent == path)
                        .map(|_| child_handle.clone())
                })
                .collect::<Vec<_>>();
            expected_children.sort();

            assert_eq!(children, expected_children);
        }
    }

    #[test]
    fn test_multiple_insertions_and_state() {
        let map = setup();

        assert!(map.handle_for_path(Path::new("a")).is_err());
        assert!(map.path_for_handle(&Handle([9; 8])).is_err());

        let h_root = map.root();
        let h_a = map.ensure_child_handle(&h_root, &child_name("a"), child_lock("a")).unwrap();
        let h_b = map.ensure_child_handle(&h_a, &child_name("b"), child_lock("a/b")).unwrap();
        let h_c = map.ensure_child_handle(&h_a, &child_name("c"), child_lock("a/c")).unwrap();
        let h_d = map.ensure_child_handle(&h_a, &child_name("d"), child_lock("a/d")).unwrap();
        let h_e = map.ensure_child_handle(&h_d, &child_name("e"), child_lock("a/d/e")).unwrap();

        assert_eq!(map.handle_for_path(Path::new("a")).unwrap(), h_a);
        assert_eq!(
            map.path_for_handle(&h_e).unwrap().try_read().unwrap().as_path(),
            Path::new("a/d/e")
        );

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

        let h_x = map.ensure_child_handle(&map.root(), &child_name("x"), child_lock("x")).unwrap();
        let h1 = map.ensure_child_handle(&h_x, &child_name("1"), child_lock("x/1")).unwrap();
        let h2 = map.ensure_child_handle(&h_x, &child_name("2"), child_lock("x/2")).unwrap();
        let h3 = map.ensure_child_handle(&h_x, &child_name("3"), child_lock("x/3")).unwrap();

        assert_eq!(map.handle_for_path(Path::new("x/2")).unwrap(), h2);
        assert_eq!(
            map.path_for_handle(&h3).unwrap().try_read().unwrap().as_path(),
            Path::new("x/3")
        );

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

        let h_dir =
            map.ensure_child_handle(&map.root(), &child_name("dir"), child_lock("dir")).unwrap();
        let h1 = map.ensure_child_handle(&h_dir, &child_name("a"), child_lock("dir/a")).unwrap();
        let h2 = map.ensure_child_handle(&h_dir, &child_name("b"), child_lock("dir/b")).unwrap();

        assert_eq!(map.handle_for_path(Path::new("dir/a")).unwrap(), h1);
        assert_eq!(
            map.path_for_handle(&h2).unwrap().try_read().unwrap().as_path(),
            Path::new("dir/b")
        );

        let h1b = map.ensure_child_handle(&h_dir, &child_name("a"), child_lock("dir/a")).unwrap();
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

        let h_p = map.ensure_child_handle(&map.root(), &child_name("p"), child_lock("p")).unwrap();
        let h1 = map.ensure_child_handle(&h_p, &child_name("a"), child_lock("p/a")).unwrap();
        let h1_child = map.ensure_child_handle(&h1, &child_name("x"), child_lock("p/a/x")).unwrap();
        let h2 = map.ensure_child_handle(&h_p, &child_name("b"), child_lock("p/b")).unwrap();

        assert_eq!(map.handle_for_path(Path::new("p/a")).unwrap(), h1);

        map.remove_path(Path::new("p/a")).unwrap();

        assert!(map.handle_for_path(Path::new("p/a")).is_err());
        assert!(map.path_for_handle(&h1).is_err());
        assert!(map.handle_for_path(Path::new("p/a/x")).is_err());
        assert!(map.path_for_handle(&h1_child).is_err());

        assert_state(
            &map,
            &[(map.root(), PathBuf::new()), (h_p, PathBuf::from("p")), (h2, PathBuf::from("p/b"))],
        );
    }

    #[test]
    fn test_rename_updates_all_tables() {
        let map = setup();

        let h_p = map.ensure_child_handle(&map.root(), &child_name("p"), child_lock("p")).unwrap();
        let h1 = map.ensure_child_handle(&h_p, &child_name("a"), child_lock("p/a")).unwrap();
        let h1_child = map.ensure_child_handle(&h1, &child_name("x"), child_lock("p/a/x")).unwrap();
        let h_z = map.ensure_child_handle(&h_p, &child_name("z"), child_lock("p/z")).unwrap();
        let h_z_child =
            map.ensure_child_handle(&h_z, &child_name("q"), child_lock("p/z/q")).unwrap();

        assert_eq!(map.handle_for_path(Path::new("p/a")).unwrap(), h1.clone());

        map.rename_path(Path::new("p/a"), Path::new("p/z"), h1.clone(), None).unwrap();

        assert!(map.handle_for_path(Path::new("p/a")).is_err());
        assert_eq!(map.handle_for_path(Path::new("p/z")).unwrap(), h1);
        assert_eq!(map.handle_for_path(Path::new("p/z/x")).unwrap(), h1_child);
        assert_eq!(
            map.path_for_handle(&h1).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z")
        );
        assert_eq!(
            map.path_for_handle(&h1_child).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z/x")
        );
        assert!(map.path_for_handle(&h_z).is_err());
        assert!(map.path_for_handle(&h_z_child).is_err());

        assert_state(
            &map,
            &[
                (map.root(), PathBuf::new()),
                (h_p, PathBuf::from("p")),
                (h1, PathBuf::from("p/z")),
                (h1_child, PathBuf::from("p/z/x")),
            ],
        );
    }

    #[test]
    fn test_parent_child_helpers() {
        let map = setup();
        let h_p = map.ensure_child_handle(&map.root(), &child_name("p"), child_lock("p")).unwrap();
        let name = child_name("a");
        let h_a = map.ensure_child_handle(&h_p, &name, child_lock("p/a")).unwrap();

        assert_eq!(map.handle_for_child(&h_p, &name).unwrap(), h_a);
        assert_eq!(
            map.path_for_child(&h_p, &name).unwrap().try_read().unwrap().as_path(),
            Path::new("p/a")
        );

        map.remove_child(&h_p, &name).unwrap();

        assert!(map.handle_for_child(&h_p, &name).is_err());
        assert!(map.path_for_handle(&h_a).is_err());
    }
}
