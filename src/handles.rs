//! HandleMap provides a bidirectional mapping between NFS file handles and
//! relative filesystem paths, together with a direct directory -> children
//! index.
//!
//! # Concurrency model
//!
//! HandleMap is not atomic with respect to multi-table updates.
//! Mutating operations update several internal structures:
//! - `handle_to_path`
//! - `path_to_handle`
//! - per-entry direct-child indexes stored in `Entry::children`
//!
//! These updates are not performed as one transaction inside `HandleMap`.
//! Instead, callers are expected to acquire the necessary external write-locks
//! before invoking any mutating operation. `HandleMap` itself does not enforce
//! that locking discipline.
//!
//! # Stored structure
//!
//! - all stored paths are relative to the configured root;
//! - `handle_to_path` stores the canonical entry for each known handle;
//! - `path_to_handle` provides the reverse lookup from relative path to handle;
//! - each `Entry` stores only its direct children in `Entry::children`.
//!
//! There is no separate recursive tree index. Recursive operations walk the
//! structure through direct-child links.
//!
//! # Mutation semantics
//!
//! - `ensure_child_handle` creates a direct child entry if it does not exist;
//! - removal is recursive and best-effort for descendants: if recursive
//!   cleanup of a nested child fails, that child-removal error is ignored;
//! - rename is implemented as "build destination subtree first, remove source
//!   subtree second";
//! - during rename, existing destination nodes are removed best-effort before a
//!   replacement node is created at that path;
//! - rename allocates fresh handles for moved nodes, so old source handles
//!   become stale after source cleanup completes.
//!
//! # Path conversion
//!
//! HandleMap stores relative paths only. Conversion to absolute filesystem
//! paths is done via [`HandleMap::to_full_path`] using the configured root.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::RwLock;

use crate::vfs;
use crate::vfs::file;
use crate::vfs::file::{Handle, Name};

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
    /// Creates a new entry with an empty direct-child index.
    fn new(handle: Handle, path: Arc<RwLock<PathBuf>>) -> Self {
        Self { path, handle, children: DashMap::new() }
    }
}

/// A bidirectional mapping between NFS file handles and relative filesystem
/// paths, plus a directory -> children index.
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
        {
            let parent_entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
            if let Some(existing_handle) =
                parent_entry.children.get(name).map(|entry| entry.value().handle.clone())
            {
                return Ok(existing_handle);
            }
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let handle = file::Handle(id.to_be_bytes());
        let mut new_path = parent_path.to_owned();
        new_path.push(name.as_str());
        let lock = Arc::new(RwLock::new(new_path.to_path_buf()));
        let entry = Entry::new(handle.clone(), lock.clone());

        // atomicity updating both tables
        let _lock_guard = lock.try_write().map_err(|_| vfs::Error::ServerFault)?;
        self.handle_to_path.insert(handle.clone(), entry);
        self.path_to_handle.insert(new_path, handle.clone());

        let parent_entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        let _ = parent_entry
            .children
            .entry(name.clone())
            .insert(Descendant { handle: handle.clone(), lock: lock.clone() });

        Ok(handle)
    }

    /// Creates a new handle for the given relative path and links it into the
    /// direct-child index of its parent.
    fn create_handle(&self, path: &std::path::Path) -> Result<Handle, vfs::Error> {
        let parent_path = path.parent().ok_or(vfs::Error::ServerFault)?;
        let name = path
            .file_name()
            .ok_or(vfs::Error::ServerFault)?
            .to_os_string()
            .into_string()
            .map_err(|_| vfs::Error::ServerFault)?;
        let name = Name::new(name).map_err(|_| vfs::Error::ServerFault)?;
        let parent_handle =
            self.path_to_handle.get(parent_path).ok_or(vfs::Error::StaleFile)?.value().clone();

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let handle = file::Handle(id.to_be_bytes());
        let lock = Arc::new(RwLock::new(path.to_path_buf()));
        let entry = Entry::new(handle.clone(), lock.clone());

        // atomicity updating both tables
        let _lock_guard = lock.try_write().map_err(|_| vfs::Error::ServerFault)?;
        self.handle_to_path.insert(handle.clone(), entry);
        self.path_to_handle.insert(path.to_path_buf(), handle.clone());

        self.handle_to_path.entry(parent_handle).and_modify(|entry| {
            entry.children.insert(name, Descendant { handle: handle.clone(), lock: lock.clone() });
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

    /// Removes the exact `path`/`handle` pair and then recursively removes all
    /// descendants reachable through the entry's direct-child index.
    ///
    /// Descendant cleanup is best-effort: errors produced while removing nested
    /// children are ignored.
    fn remove_entry_recursive(&self, path: &Path, handle: &Handle) -> Result<(), vfs::Error> {
        // since locks are held from outside, updating of both functions are atomic
        self.path_to_handle.remove(path).ok_or(vfs::Error::StaleFile)?;
        let (_, entry) = self.handle_to_path.remove(handle).ok_or(vfs::Error::StaleFile)?;
        let children = entry
            .children
            .iter()
            .map(|child| (child.key().clone(), child.handle.clone()))
            .collect::<Vec<(file::Name, Handle)>>();

        for (name, child_handle) in children {
            let mut new_path = path.to_path_buf();
            new_path.push(name.as_str());
            // ignore subtree removing errors
            let _ = self.remove_entry_recursive(new_path.as_path(), &child_handle);
        }
        Ok(())
    }

    /// Resolves a relative path to its parent handle and final component, then
    /// removes that entry recursively.
    fn remove_path_by_path(&self, path: &Path) -> Result<(), vfs::Error> {
        let parent = self.get_parent_handle(path)?;
        let name = path
            .file_name()
            .ok_or(vfs::Error::StaleFile)?
            .to_os_string()
            .into_string()
            .map_err(|_| vfs::Error::ServerFault)?;
        let name = Name::new(name).map_err(|_| vfs::Error::ServerFault)?;
        self.remove_path(path, &parent, &name)
    }

    /// Removes a direct child from `parent` by name and recursively deletes the
    /// subtree rooted at `path`.
    fn remove_path(
        &self,
        path: &Path,
        parent: &Handle,
        name: &file::Name,
    ) -> Result<(), vfs::Error> {
        let descendant = {
            let parent_entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
            let (_, descendant) =
                parent_entry.children.remove(name).ok_or(vfs::Error::StaleFile)?;
            descendant
        };
        self.remove_entry_recursive(path, &descendant.handle)
    }

    /// Renames a subtree by creating a new subtree at the destination.
    ///
    /// If the source and destination are identical, this is a no-op and the
    /// existing child handle is returned.
    ///
    /// Existing destination entries are removed recursively before creating
    /// each destination node. The original source subtree is removed only after
    /// the destination subtree has been built successfully. This operation
    /// allocates fresh handles for the new subtree, so old source handles
    /// become stale after source cleanup.
    ///
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
        if from_path == to_path && from_name == to_name {
            return self.handle_for_child(from_parent, from_name);
        }
        let source_handle = {
            let parent_entry = self.handle_to_path.get(from_parent).ok_or(vfs::Error::StaleFile)?;
            let (_, source_handle) =
                parent_entry.children.remove(from_name).ok_or(vfs::Error::StaleFile)?;
            source_handle
        };

        let mut old_path = from_path.to_path_buf();
        old_path.push(from_name.as_str());

        let mut new_path = to_path.to_path_buf();
        new_path.push(to_name.as_str());

        let handle = self.rename_entry_recursive(
            old_path.as_path(),
            new_path.as_path(),
            &source_handle.handle,
        )?;
        self.remove_entry_recursive(old_path.as_path(), &source_handle.handle)?;
        Ok(handle)
    }

    // Create a new subtree at `new_path` by walking the source subtree rooted
    // at `old_handle`. Existing destination nodes are removed best-effort
    // before each new node is created.
    fn rename_entry_recursive(
        &self,
        old_path: &Path,
        new_path: &Path,
        old_handle: &Handle,
    ) -> Result<Handle, vfs::Error> {
        let children = {
            let old_entry = self.handle_to_path.get(old_handle).ok_or(vfs::Error::StaleFile)?;
            old_entry
                .children
                .iter()
                .map(|entry| (entry.key().clone(), entry.handle.clone()))
                .collect::<Vec<(file::Name, Handle)>>()
        };

        // remove if there are already something exist on paths we are trying to create
        let _ = self.remove_path_by_path(new_path);

        // there should be parent -
        // and there would definitely be one, since we do up-to-buttom
        let handle = self.create_handle(new_path)?;
        for (name, child_handle) in children {
            let mut old_path = old_path.to_path_buf();
            let mut new_path = new_path.to_path_buf();
            old_path.push(name.as_str());
            new_path.push(name.as_str());
            self.rename_entry_recursive(old_path.as_path(), new_path.as_path(), &child_handle)?;
        }
        Ok(handle)
    }

    /// Collects path locks for the subtree rooted at `handle`, including the
    /// root node itself.
    pub fn collect_locks_from_subtree(
        &self,
        handle: &Handle,
    ) -> Result<Vec<Arc<RwLock<PathBuf>>>, vfs::Error> {
        let mut acc = Vec::new();
        self.collect_locks_with_acc(&mut acc, handle)?;
        Ok(acc)
    }

    /// Recursively collects path locks into `acc` for the subtree rooted at
    /// `handle`.
    fn collect_locks_with_acc(
        &self,
        acc: &mut Vec<Arc<RwLock<PathBuf>>>,
        handle: &Handle,
    ) -> Result<(), vfs::Error> {
        let entry = self.handle_to_path.get(handle).ok_or(vfs::Error::StaleFile)?;
        acc.push(entry.path.clone());
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
                (h_p.clone(), PathBuf::from("p"), from_ref(&h2)),
                (h2.clone(), PathBuf::from("p/b"), &[]),
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
            map.rename_path(&h_p, Path::new("p"), Path::new("p"), &name_a, &name_z).unwrap();
        let renamed_child = map.handle_for_child(&renamed, &name_x).unwrap();

        assert!(map.handle_for_child(&h_p, &name_a).is_err());
        assert_eq!(map.handle_for_child(&h_p, &name_z).unwrap(), renamed);
        assert_eq!(
            map.path_for_child(&h_p, &name_z).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z")
        );
        assert_ne!(renamed_child, h1_child);
        assert_eq!(
            map.path_for_handle(&renamed).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z")
        );
        assert_eq!(
            map.path_for_handle(&renamed_child).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z/x")
        );
        assert!(matches!(map.path_for_handle(&h1), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_handle(&h1_child), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_handle(&h_z), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_handle(&h_z_child), Err(vfs::Error::StaleFile)));

        assert_state(
            &map,
            &[
                (HandleMap::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), from_ref(&renamed)),
                (renamed.clone(), PathBuf::from("p/z"), from_ref(&renamed_child.clone())),
                (renamed_child, PathBuf::from("p/z/x"), &[]),
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
            map.rename_path(&h_p, Path::new("p"), Path::new("p"), &name_a, &name_z).unwrap();
        let renamed_child = map.handle_for_child(&renamed, &name_x).unwrap();

        assert_eq!(map.handle_for_child(&h_p, &name_z).unwrap(), renamed);
        assert_ne!(map.handle_for_child(&renamed, &name_x).unwrap(), h_a_child);

        assert!(map.handle_for_child(&h_p, &name_a).is_err());
        assert!(matches!(map.path_for_handle(&h_a), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_handle(&h_z), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_handle(&h_z_child), Err(vfs::Error::StaleFile)));
        assert_eq!(
            map.path_for_handle(&renamed).unwrap().try_read().unwrap().as_path(),
            Path::new("p/z")
        );
        map.path_for_handle(&renamed_child).unwrap();
        assert!(matches!(map.path_for_handle(&h_a_child), Err(vfs::Error::StaleFile)));

        let renamed2 =
            map.rename_path(&h_p, Path::new("p"), Path::new("p"), &name_z, &name_z).unwrap();

        assert_eq!(renamed, renamed2);
        assert_state(
            &map,
            &[
                (HandleMap::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), from_ref(&renamed)),
                (renamed.clone(), PathBuf::from("p/z"), from_ref(&renamed_child.clone())),
                (renamed_child, PathBuf::from("p/z/x"), &[]),
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
