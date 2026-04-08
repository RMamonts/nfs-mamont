//! HandleMap stores a bidirectional mapping between NFS file handles and
//! relative filesystem paths, plus a direct directory → children index.
//!
//! # Concurrency model
//!
//! HandleMap is intentionally non-atomic across multi-structure updates.
//! The internal operations only rewire handles, path locks, and parent →
//! child links; they do not take `RwLock` guards themselves.
//!
//! Callers must acquire the relevant write-locks before invoking mutating
//! operations. That external locking protocol provides the atomicity that this
//! structure does not enforce on its own.
//!
//! # Semantics
//!
//! - paths are stored relative to the configured root;
//! - `path_for_handle` and `path_for_child` return the stored path locks;
//! - `ensure_child_handle` creates a direct child entry if it does not exist;
//! - `remove_child` and `rename_path` operate on a single element only;
//! - descendants are left untouched and remain valid through their own handles.

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
struct Descendant {
    handle: Handle,
    lock: Arc<RwLock<PathBuf>>,
}

struct Entry {
    path: Arc<RwLock<PathBuf>>,
    handle: Handle,
    children: DashMap<file::Name, Descendant>,
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
    handle_to_path: DashMap<Handle, Entry>,
    path_to_handle: DashMap<PathBuf, Handle>,
    next_id: AtomicU64,
}

impl HandleMap {
    /// Creates a new handle map rooted at the given absolute path.
    pub fn new(root: PathBuf) -> Self {
        let root_handle = file::Handle(ROOT.to_be_bytes());
        let root_relative = PathBuf::new();
        let root_lock = Arc::new(RwLock::new(root_relative.clone()));

        let handle_to_path = DashMap::new();
        handle_to_path.insert(root_handle.clone(), Entry::new(root_handle.clone(), root_lock));

        let path_to_handle = DashMap::new();
        path_to_handle.insert(root_relative.clone(), root_handle);

        Self { root, handle_to_path, path_to_handle, next_id: AtomicU64::new(ROOT + 1) }
    }

    /// Returns the fixed handle representing the root directory.
    pub fn root() -> file::Handle {
        file::Handle(ROOT.to_be_bytes())
    }

    /// Returns the stored path lock for a handle.
    pub fn path_for_handle(
        &self,
        handle: &file::Handle,
    ) -> Result<Arc<RwLock<PathBuf>>, vfs::Error> {
        Ok(self.handle_to_path.get(handle).ok_or(vfs::Error::StaleFile)?.value().path.clone())
    }

    /// Returns the stored path lock for a direct child entry.
    pub fn path_for_child(
        &self,
        parent: &file::Handle,
        name: &file::Name,
    ) -> Result<Arc<RwLock<PathBuf>>, vfs::Error> {
        let parent_entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        let child = parent_entry.children.get(name).ok_or(vfs::Error::StaleFile)?;
        Ok(child.value().lock.clone())
    }

    /// Returns the direct child handle for a parent and entry name.
    pub fn handle_for_child(
        &self,
        parent: &file::Handle,
        name: &file::Name,
    ) -> Result<Handle, vfs::Error> {
        let parent_entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        let child = parent_entry.children.get(name).ok_or(vfs::Error::StaleFile)?;
        Ok(child.value().handle.clone())
    }

    /// Returns the direct child handle or creates a new direct child entry.
    ///
    /// The caller must already hold any required external write-locks for the
    /// parent path and the child path prefix. The provided `parent_path` is used
    /// to construct the new relative path lock for the child.
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

        self.handle_to_path.insert(handle.clone(), entry);
        self.path_to_handle.insert(new_path, handle.clone());

        let parent_entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        parent_entry.children.insert(name.clone(), Descendant { handle: handle.clone(), lock });

        Ok(handle)
    }

    /// Converts a relative path into an absolute path under the configured root.
    fn to_full_path(&self, relative: &Path) -> PathBuf {
        self.root.join(relative)
    }

    /// Removes one direct child entry and drops its handle from the map.
    fn remove_child(
        &self,
        parent_path: &Path,
        parent: &Handle,
        name: &file::Name,
    ) -> Result<(), vfs::Error> {
        // root cannot be removed since it has no parent
        let parent_entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        let (_, entry) = parent_entry.children.remove(name).ok_or(vfs::Error::StaleFile)?;
        self.handle_to_path.remove(&entry.handle).ok_or(vfs::Error::StaleFile)?;
        self.path_to_handle.remove(parent_path).ok_or(vfs::Error::StaleFile)?;
        Ok(())
    }

    fn get_handle_by_path(&self, path: &Path) -> Result<Handle, vfs::Error> {
        let path = self.path_to_handle.get(path).ok_or(vfs::Error::StaleFile)?;
        Ok(path.value().clone())
    }

    fn get_parent_handle(&self, path: &Path) -> Result<Handle, vfs::Error> {
        if self.to_full_path(path) == self.root {
            return Ok(Self::root());
        }
        let path = path.parent().ok_or(vfs::Error::StaleFile)?;
        self.get_handle_by_path(path)
    }

    /// Renames a single entry and leaves descendants untouched.
    ///
    /// The destination entry is created first, then the source entry is removed.
    /// Callers must hold the relevant external write-locks before calling this
    /// method.
    pub fn rename_path(
        &self,
        from_parent: &Handle,
        to_parent: &Handle,
        from_path: &Path,
        to_path: &Path,
        from_name: &file::Name,
        to_name: &file::Name,
    ) -> Result<Handle, vfs::Error> {
        // root cannot be renamed since it has no parent
        if from_parent == to_parent && from_name == to_name {
            return self.handle_for_child(from_parent, from_name);
        }

        // make sure nobody would interact with previous object
        let _ = self.remove_child(from_path, to_parent, to_name);
        let new_handle = self.ensure_child_handle(to_path, to_parent, to_name)?;
        match self.remove_child(from_path, from_parent, from_name) {
            Ok(()) => Ok(new_handle),
            Err(err) => {
                let _ = self.remove_child(to_path, to_parent, to_name);
                Err(err)
            }
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
        "" | "." => Err(vfs::Error::InvalidArgument),
        ".." => Err(vfs::Error::Exist),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::array::from_ref;
    use std::path::{Path, PathBuf};

    /// Creates a fresh HandleMap with a dummy root.
    fn setup() -> HandleMap {
        HandleMap::new(PathBuf::from("/tmp"))
    }

    fn child_name(name: &str) -> file::Name {
        file::Name::new(name.to_string()).unwrap()
    }

    /// Asserts that the tree stored in HandleMap matches the expected state.
    fn assert_state(map: &HandleMap, exp: &[(Handle, PathBuf, &[Handle])]) {
        let mut actual = map
            .handle_to_path
            .iter()
            .map(|entry| {
                let path = entry.value().path.try_read().unwrap().clone();
                (entry.key().clone(), path)
            })
            .collect::<Vec<(Handle, PathBuf)>>();

        actual.sort_by(|(left, _), (right, _)| left.cmp(right));

        let mut expected = exp
            .iter()
            .map(|(handle, path, _)| (handle.clone(), path.clone()))
            .collect::<Vec<(Handle, PathBuf)>>();
        expected.sort_by(|(left, _), (right, _)| left.cmp(right));

        assert_eq!(actual, expected);

        for (handle, _, children) in exp {
            let Some(entry) = map.handle_to_path.get(handle) else {
                panic!("missing handle {handle:?}");
            };

            let mut actual_children = entry
                .children
                .iter()
                .map(|child| child.value().handle.clone())
                .collect::<Vec<Handle>>();
            actual_children.sort();

            let mut expected_children = children.to_vec();
            expected_children.sort();

            assert_eq!(actual_children, expected_children);
        }
    }

    #[test]
    fn test_multiple_insertions_and_state() {
        let map = setup();

        assert!(map.path_for_handle(&Handle([9; 8])).is_err());

        let h_root = HandleMap::root();
        let name_a = child_name("a");
        let name_b = child_name("b");
        let name_c = child_name("c");
        let name_d = child_name("d");
        let name_e = child_name("e");

        assert!(map.handle_for_child(&h_root, &name_a).is_err());
        let h_a = map.ensure_child_handle(Path::new(""), &h_root, &name_a).unwrap();
        let h_b = map.ensure_child_handle(Path::new("a"), &h_a, &name_b).unwrap();
        let h_b2 = map.ensure_child_handle(Path::new("a"), &h_a, &name_b).unwrap();
        let h_c = map.ensure_child_handle(Path::new("a"), &h_a, &name_c).unwrap();
        let h_d = map.ensure_child_handle(Path::new("a"), &h_a, &name_d).unwrap();
        let h_e = map.ensure_child_handle(Path::new("a/d"), &h_d, &name_e).unwrap();

        assert_eq!(h_b, h_b2);
        assert_eq!(map.handle_for_child(&h_a, &name_d).unwrap(), h_d);
        assert_eq!(
            map.path_for_handle(&h_e).unwrap().try_read().unwrap().as_path(),
            Path::new("a/d/e")
        );

        assert_state(
            &map,
            &[
                (h_root, PathBuf::new(), from_ref(&h_a)),
                (h_a.clone(), PathBuf::from("a"), &[h_b.clone(), h_c.clone(), h_d.clone()]),
                (h_b, PathBuf::from("a/b"), &[]),
                (h_c, PathBuf::from("a/c"), &[]),
                (h_d, PathBuf::from("a/d"), from_ref(&h_e)),
                (h_e.clone(), PathBuf::from("a/d/e"), &[]),
            ],
        );
    }

    #[test]
    fn test_children_population() {
        let map = setup();

        let name_x = child_name("x");
        let name_1 = child_name("1");
        let name_2 = child_name("2");
        let name_3 = child_name("3");

        assert!(map.handle_for_child(&HandleMap::root(), &name_x).is_err());
        let h_x = map.ensure_child_handle(Path::new(""), &HandleMap::root(), &name_x).unwrap();
        let h1 = map.ensure_child_handle(Path::new("x"), &h_x, &name_1).unwrap();
        let h2 = map.ensure_child_handle(Path::new("x"), &h_x, &name_2).unwrap();
        let h3 = map.ensure_child_handle(Path::new("x"), &h_x, &name_3).unwrap();

        assert_eq!(map.handle_for_child(&h_x, &name_2).unwrap(), h2);
        assert_eq!(map.handle_for_child(&h_x, &name_3).unwrap(), h3);
        assert_eq!(
            map.path_for_handle(&h3).unwrap().try_read().unwrap().as_path(),
            Path::new("x/3")
        );

        let mut children = vec![
            map.handle_for_child(&h_x, &name_1).unwrap(),
            map.handle_for_child(&h_x, &name_2).unwrap(),
            map.handle_for_child(&h_x, &name_3).unwrap(),
        ];
        children.sort();
        let mut expected = vec![h1.clone(), h2.clone(), h3.clone()];
        expected.sort();

        assert_eq!(children, expected);

        assert_state(
            &map,
            &[
                (HandleMap::root(), PathBuf::new(), from_ref(&h_x)),
                (h_x.clone(), PathBuf::from("x"), &[h1.clone(), h2.clone(), h3.clone()]),
                (h1, PathBuf::from("x/1"), &[]),
                (h2, PathBuf::from("x/2"), &[]),
                (h3, PathBuf::from("x/3"), &[]),
            ],
        );
    }

    #[test]
    fn test_existing_files_and_state() {
        let map = setup();

        let name_dir = child_name("dir");
        let name_a = child_name("a");
        let name_b = child_name("b");
        assert!(map.handle_for_child(&HandleMap::root(), &name_dir).is_err());
        let h_dir = map.ensure_child_handle(Path::new(""), &HandleMap::root(), &name_dir).unwrap();
        let h1 = map.ensure_child_handle(Path::new("dir"), &h_dir, &name_a).unwrap();
        let h2 = map.ensure_child_handle(Path::new("dir"), &h_dir, &name_b).unwrap();

        assert_eq!(map.handle_for_child(&h_dir, &name_a).unwrap(), h1);
        assert_eq!(
            map.path_for_child(&h_dir, &name_b).unwrap().try_read().unwrap().as_path(),
            Path::new("dir/b")
        );

        let h1b = map.ensure_child_handle(Path::new("dir"), &h_dir, &name_a).unwrap();
        assert_eq!(h1, h1b);

        assert_state(
            &map,
            &[
                (HandleMap::root(), PathBuf::new(), from_ref(&h_dir)),
                (h_dir.clone(), PathBuf::from("dir"), &[h1.clone(), h2.clone()]),
                (h1, PathBuf::from("dir/a"), &[]),
                (h2, PathBuf::from("dir/b"), &[]),
            ],
        );
    }

    #[test]
    fn test_remove_child_updates_all_tables() {
        let map = setup();

        let name_p = child_name("p");
        let name_a = child_name("a");
        let name_b = child_name("b");
        let name_x = child_name("x");
        let h_p = map.ensure_child_handle(Path::new(""), &HandleMap::root(), &name_p).unwrap();
        let h1 = map.ensure_child_handle(Path::new("p"), &h_p, &name_a).unwrap();
        let h1_child = map.ensure_child_handle(Path::new("p/a"), &h1, &name_x).unwrap();
        let h2 = map.ensure_child_handle(Path::new("p"), &h_p, &name_b).unwrap();

        assert_eq!(map.handle_for_child(&h_p, &name_a).unwrap(), h1);

        map.remove_child(&h_p, &name_a).unwrap();

        assert!(map.handle_for_child(&h_p, &name_a).is_err());
        assert!(map.path_for_child(&h_p, &name_a).is_err());
        assert!(map.path_for_handle(&h1).is_err());
        assert!(map.path_for_child(&h1, &name_x).is_err());
        assert_eq!(
            map.path_for_handle(&h1_child).unwrap().try_read().unwrap().as_path(),
            Path::new("p/a/x")
        );

        assert_state(
            &map,
            &[
                (HandleMap::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), from_ref(&h2)),
                (h1_child, PathBuf::from("p/a/x"), &[]),
                (h2.clone(), PathBuf::from("p/b"), &[]),
            ],
        );
    }

    #[test]
    fn test_rename_path_updates_single_element() {
        let map = setup();

        let name_p = child_name("p");
        let name_a = child_name("a");
        let name_x = child_name("x");
        let name_z = child_name("z");
        let name_q = child_name("q");
        let h_p = map.ensure_child_handle(Path::new(""), &HandleMap::root(), &name_p).unwrap();
        let h1 = map.ensure_child_handle(Path::new("p"), &h_p, &name_a).unwrap();
        let h1_child = map.ensure_child_handle(Path::new("p/a"), &h1, &name_x).unwrap();
        let h_z = map.ensure_child_handle(Path::new("p"), &h_p, &name_z).unwrap();
        let h_z_child = map.ensure_child_handle(Path::new("p/z"), &h_z, &name_q).unwrap();

        assert_eq!(map.handle_for_child(&h_p, &name_a).unwrap(), h1);

        let renamed = map.rename_path(&h_p, &h_p, Path::new("p"), &name_a, &name_z).unwrap();

        assert!(map.handle_for_child(&h_p, &name_a).is_err());
        assert_eq!(map.handle_for_child(&h_p, &name_z).unwrap(), renamed);
        assert_eq!(
            map.path_for_child(&h_p, &name_z).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z")
        );
        assert!(map.handle_for_child(&renamed, &name_x).is_err());
        assert_eq!(
            map.path_for_handle(&renamed).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z")
        );
        assert_eq!(
            map.path_for_handle(&h1_child).unwrap().try_read().unwrap().as_path(),
            Path::new("p/a/x")
        );
        assert!(map.path_for_handle(&h1).is_err());
        assert!(map.path_for_handle(&h_z).is_err());
        assert_eq!(
            map.path_for_handle(&h_z_child).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z/q")
        );

        assert_state(
            &map,
            &[
                (HandleMap::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), from_ref(&renamed)),
                (renamed.clone(), PathBuf::from("p/z"), &[]),
                (h1_child, PathBuf::from("p/a/x"), &[]),
                (h_z_child, PathBuf::from("p/z/q"), &[]),
            ],
        );
    }

    #[test]
    fn test_rename_path_replaces_existing_destination() {
        let map = setup();

        let name_p = child_name("p");
        let name_a = child_name("a");
        let name_z = child_name("z");
        let h_p = map.ensure_child_handle(Path::new(""), &HandleMap::root(), &name_p).unwrap();
        let h_a = map.ensure_child_handle(Path::new("p"), &h_p, &name_a).unwrap();
        let h_z = map.ensure_child_handle(Path::new("p"), &h_p, &name_z).unwrap();

        let renamed = map.rename_path(&h_p, &h_p, Path::new("p"), &name_a, &name_z).unwrap();

        assert_eq!(map.handle_for_child(&h_p, &name_z).unwrap(), renamed);

        assert!(map.handle_for_child(&h_p, &name_a).is_err());
        assert!(map.path_for_handle(&h_a).is_err());
        assert!(map.path_for_handle(&h_z).is_err());
        assert_eq!(
            map.path_for_handle(&renamed).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z")
        );

        let renamed2 = map.rename_path(&h_p, &h_p, Path::new("p"), &name_z, &name_z).unwrap();

        assert_eq!(renamed, renamed2);

        assert_state(
            &map,
            &[
                (HandleMap::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), from_ref(&renamed)),
                (renamed.clone(), PathBuf::from("p/z"), &[]),
            ],
        );
    }

    #[test]
    fn test_parent_child_helpers() {
        let map = setup();
        let h_p =
            map.ensure_child_handle(Path::new(""), &HandleMap::root(), &child_name("p")).unwrap();
        let name = child_name("a");
        let h_a = map.ensure_child_handle(Path::new("p"), &h_p, &name).unwrap();

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
