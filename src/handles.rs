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
//! - `path_to_handle` mirrors `handle_to_path` for direct path lookups;
//! - `ensure_child_handle` creates a direct child entry if it does not exist;
//! - `remove_path` removes an entry recursively with its descendants;
//! - `remove_child` removes a single direct child entry;
//! - `rename_path` renames a subtree and rewrites descendant paths;
//! - descendants remain valid through their own handles after recursive updates.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::RwLock;

use crate::vfs;
use crate::vfs::file;
use crate::vfs::file::{Handle, Name, Path};

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
    /// to construct the new relative path lock for the child. Both handle and
    /// path lookup tables are updated for the new entry.
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

    fn create_handle(
        &self,
        path: &std::path::Path,
    ) -> Result<Handle, vfs::Error> {
        let parent_path = path.parent().ok_or(vfs::Error::ServerFault)?;
        let name = path.file_name().ok_or(vfs::Error::ServerFault)?.to_os_string().into_string().map_err(|_| vfs::Error::ServerFault)?;
        let name = Name::new(name).map_err(|_| vfs::Error::ServerFault)?;
        let parent_handle = self.path_to_handle.get(parent_path).ok_or(vfs::Error::StaleFile)?.value().clone();

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let handle = file::Handle(id.to_be_bytes());
        let lock = Arc::new(RwLock::new(path.to_path_buf()));
        let entry = Entry::new(handle.clone(), lock.clone());

        self.handle_to_path.insert(handle.clone(), entry);
        self.path_to_handle.insert(path.to_path_buf(), handle.clone());

        self.handle_to_path.entry(parent_handle).and_modify(|entry| {
            entry.children.insert(name, Descendant { handle: handle.clone(), lock });
        });

        Ok(handle)
    }

    /// Converts a relative path into an absolute path under the configured root.
    fn to_full_path(&self, relative: &Path) -> PathBuf {
        self.root.join(relative)
    }

    /// Resolves a relative path through the direct path-to-handle index.
    fn get_handle_by_path(&self, path: &Path) -> Result<Handle, vfs::Error> {
        let path = self.path_to_handle.get(path).ok_or(vfs::Error::StaleFile)?;
        Ok(path.value().clone())
    }

    /// Resolves the parent handle for a relative path.
    fn get_parent_handle(&self, path: &Path) -> Result<Handle, vfs::Error> {
        if self.to_full_path(path) == self.root {
            return Ok(Self::root());
        }
        let path = path.parent().ok_or(vfs::Error::StaleFile)?;
        self.get_handle_by_path(path)
    }

    fn remove_entry_recursive(&self, path: &Path, handle: &Handle) -> Result<(), vfs::Error> {
        self.path_to_handle.remove(path).ok_or(vfs::Error::StaleFile)?;
        let (_, entry) = self.handle_to_path.remove(&handle).ok_or(vfs::Error::StaleFile)?;

        for entry in entry.children.iter() {
            let mut new_path = path.to_path_buf();
            new_path.push(entry.key().as_str());
            // ignore subtree removing errors
            let _ = self.remove_entry_recursive(new_path.as_path(), &entry.handle);
        }
        Ok(())
    }

    /// Removes a path entry together with all of its descendants.
    pub fn remove_path(
        &self,
        path: &Path,
        parent: &Handle,
        name: &file::Name,
    ) -> Result<(), vfs::Error> {
        let parent_entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        let (_, entry) = parent_entry.children.remove(name).ok_or(vfs::Error::StaleFile)?;
        // error would be returned, only if problem with name - subtree errors ignored
        self.remove_entry_recursive(path, &entry.handle)
    }

    /// Renames a subtree and rewrites descendant paths.
    ///
    /// If the destination already exists, it is removed recursively first.
    /// Callers must hold the relevant external write-locks before calling this
    /// method.
    pub fn rename_path(
        &self,
        from_parent: &Handle,
        from_path: &std::path::Path,
        to_path: &std::path::Path,
        from_name: &file::Name,
        to_name: &file::Name,
    ) -> Result<Handle, vfs::Error> {
        let parent_entry = self.handle_to_path.get(from_parent).ok_or(vfs::Error::StaleFile)?;
        let (_, entry) = parent_entry.children.remove(from_name).ok_or(vfs::Error::StaleFile)?;

        let mut old_path = from_path.to_path_buf();
        old_path.push(from_name.as_str());

        let mut new_path = to_path.to_path_buf();
        new_path.push(to_name.as_str());

        let handle = self.rename_entry_recursive(old_path.as_path(), new_path.as_path(), from_parent)?;
        self.remove_entry_recursive(old_path.as_path(), &entry.handle)?;
        Ok(handle)
    }

    // create subtree copy on new path
    fn rename_entry_recursive(
        &self,
        old_path: &Path,
        new_path: &Path,
        old_handle: &Handle,
    ) -> Result<Handle, vfs::Error> {
        let (_, old_entry) =
            self.handle_to_path.remove(old_handle).ok_or(vfs::Error::StaleFile)?;
        self.path_to_handle.remove(old_path).ok_or(vfs::Error::StaleFile)?;
        // clean_existing
        let _ = self
            .path_to_handle
            .remove(new_path)
            .and_then(|(_, handle)| self.handle_to_path.remove(&handle));

        // there should be parent -
        // and there would definitely be one, since we do up-to-buttom
        let handle = self.create_handle(new_path)?;
        for entry in old_entry.children.iter() {
            let mut old_path = old_path.to_path_buf();
            let mut new_path = new_path.to_path_buf();
            old_path.push(entry.key().as_str());
            new_path.push(entry.key().as_str());
            self.rename_entry_recursive(
                old_path.as_path(),
                new_path.as_path(),
                &old_entry.handle,
            )?;
        }
        Ok(handle)
    }

    pub fn collect_locks_from_subtree(&self, handle: &Handle) -> Result<Vec<Arc<RwLock<PathBuf>>>, vfs::Error> {
        let mut acc = Vec::new();
        self.collect_locks_with_acc(&mut acc, handle)?;
        Ok(acc)
    }

    fn collect_locks_with_acc(&self, acc: &mut Vec<Arc<RwLock<PathBuf>>>, handle: &Handle) -> Result<(), vfs::Error>  {
        let entry = self.handle_to_path.get(handle).ok_or(vfs::Error::StaleFile)?;
        let _ = entry.children.iter().map(|child| acc.push(child.lock.clone()));
        for child in entry.children.iter() {
            self.collect_locks_with_acc(acc, &child.handle)?;
        }
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

        let mut actual_paths = map
            .path_to_handle
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect::<Vec<(PathBuf, Handle)>>();
        actual_paths.sort_by(|(left, _), (right, _)| left.cmp(right));

        let mut expected_paths = exp
            .iter()
            .map(|(handle, path, _)| (path.clone(), handle.clone()))
            .collect::<Vec<(PathBuf, Handle)>>();
        expected_paths.sort_by(|(left, _), (right, _)| left.cmp(right));

        assert_eq!(actual_paths, expected_paths);

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

        map.remove_path(Path::new("p/a"), &h_p, &name_a).unwrap();

        assert!(map.handle_for_child(&h_p, &name_a).is_err());
        assert!(map.path_for_child(&h_p, &name_a).is_err());
        assert!(map.path_for_handle(&h1).is_err());
        assert!(map.path_for_child(&h1, &name_x).is_err());
        assert!(map.path_for_handle(&h1_child).is_err());

        assert_state(
            &map,
            &[
                (HandleMap::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), from_ref(&h2)),
                (h2.clone(), PathBuf::from("p/b"), &[]),
            ],
        );
    }

    #[test]
    fn test_remove_path_removes_subtree() {
        let map = setup();

        let name_p = child_name("p");
        let name_a = child_name("a");
        let name_b = child_name("b");
        let name_x = child_name("x");
        let h_p = map.ensure_child_handle(Path::new(""), &HandleMap::root(), &name_p).unwrap();
        let h1 = map.ensure_child_handle(Path::new("p"), &h_p, &name_a).unwrap();
        let h1_child = map.ensure_child_handle(Path::new("p/a"), &h1, &name_x).unwrap();
        let h2 = map.ensure_child_handle(Path::new("p"), &h_p, &name_b).unwrap();

        map.remove_path(Path::new("p/a"), &h_p, &name_a).unwrap();

        assert!(map.handle_for_child(&h_p, &name_a).is_err());
        assert!(map.path_for_handle(&h1).is_err());
        assert!(map.path_for_handle(&h1_child).is_err());
        assert_eq!(map.handle_for_child(&h_p, &name_b).unwrap(), h2);

        assert_state(
            &map,
            &[
                (HandleMap::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), &[h2.clone()]),
                (h2, PathBuf::from("p/b"), &[]),
            ],
        );
    }

    #[test]
    fn test_rename_path_updates_subtree() {
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

        let renamed =
            map.rename_path(&h_p, &h_p, Path::new("p"), Path::new("p"), &name_a, &name_z).unwrap();

        assert!(map.handle_for_child(&h_p, &name_a).is_err());
        assert_eq!(map.handle_for_child(&h_p, &name_z).unwrap(), renamed);
        assert_eq!(
            map.path_for_child(&h_p, &name_z).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z")
        );
        assert_eq!(map.handle_for_child(&renamed, &name_x).unwrap(), h1_child);
        assert_eq!(
            map.path_for_handle(&renamed).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z")
        );
        assert_eq!(
            map.path_for_handle(&h1_child).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z/x")
        );
        assert_eq!(
            map.path_for_handle(&h1).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z")
        );
        assert!(map.path_for_handle(&h_z).is_err());
        assert!(map.path_for_handle(&h_z_child).is_err());

        assert_state(
            &map,
            &[
                (HandleMap::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), from_ref(&renamed)),
                (renamed.clone(), PathBuf::from("p/z"), &[h1_child.clone()]),
                (h1_child, PathBuf::from("p/z/x"), &[]),
            ],
        );
    }

    #[test]
    fn test_rename_path_replaces_existing_destination() {
        let map = setup();

        let name_p = child_name("p");
        let name_a = child_name("a");
        let name_x = child_name("x");
        let name_z = child_name("z");
        let name_q = child_name("q");
        let h_p = map.ensure_child_handle(Path::new(""), &HandleMap::root(), &name_p).unwrap();
        let h_a = map.ensure_child_handle(Path::new("p"), &h_p, &name_a).unwrap();
        let h_a_child = map.ensure_child_handle(Path::new("p/a"), &h_a, &name_x).unwrap();
        let h_z = map.ensure_child_handle(Path::new("p"), &h_p, &name_z).unwrap();
        let h_z_child = map.ensure_child_handle(Path::new("p/z"), &h_z, &name_q).unwrap();

        let renamed =
            map.rename_path(&h_p, &h_p, Path::new("p"), Path::new("p"), &name_a, &name_z).unwrap();

        assert_eq!(map.handle_for_child(&h_p, &name_z).unwrap(), renamed);
        assert_eq!(map.handle_for_child(&renamed, &name_x).unwrap(), h_a_child);

        assert!(map.handle_for_child(&h_p, &name_a).is_err());
        assert_eq!(
            map.path_for_handle(&h_a).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z")
        );
        assert!(map.path_for_handle(&h_z).is_err());
        assert!(map.path_for_handle(&h_z_child).is_err());
        assert_eq!(
            map.path_for_handle(&renamed).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z")
        );
        assert_eq!(
            map.path_for_handle(&h_a_child).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z/x")
        );

        let renamed2 =
            map.rename_path(&h_p, &h_p, Path::new("p"), Path::new("p"), &name_z, &name_z).unwrap();

        assert_eq!(renamed, renamed2);

        assert_state(
            &map,
            &[
                (HandleMap::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), from_ref(&renamed)),
                (renamed.clone(), PathBuf::from("p/z"), &[h_a_child.clone()]),
                (h_a_child, PathBuf::from("p/z/x"), &[]),
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

        map.remove_path(Path::new("p/a"), &h_p, &name).unwrap();

        assert!(map.handle_for_child(&h_p, &name).is_err());
        assert!(map.path_for_handle(&h_a).is_err());
    }
}
