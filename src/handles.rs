//! HandleMap provides a bidirectional mapping between NFS file handles and
//! relative filesystem paths, together with a direct directory -> children
//! index.
//!
//! # Concurrency model
//!
//! All state is protected by a single [`std::sync::RwLock`]. Read-only
//! lookups ([`HandleMap::path_for_handle`], [`HandleMap::handle_for_child`],
//! etc.) acquire a shared read lock and may run concurrently. Mutating
//! operations ([`HandleMap::ensure_child_handle`], [`HandleMap::remove_path`],
//! [`HandleMap::rename_path`]) acquire an exclusive write lock, ensuring that
//! all internal tables are updated atomically.
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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::vfs;
use crate::vfs::file;
use crate::vfs::file::{Handle, Name};

const ROOT: u64 = 1;

/// A directory entry in the handle map.
struct Entry {
    path: PathBuf,
    handle: Handle,
    children: HashMap<file::Name, Handle>,
}

impl Entry {
    /// Creates a new entry with the given handle and path.
    fn new(handle: Handle, path: PathBuf) -> Self {
        Self { path, handle, children: HashMap::new() }
    }
}

/// The inner state of the handle map.
struct Inner {
    root: PathBuf,
    handle_to_path: HashMap<Handle, Entry>,
    path_to_handle: HashMap<PathBuf, Handle>,
    next_id: u64,
}

impl Inner {
    /// Returns the handle for the given path.
    fn get_handle_by_path(&self, path: &Path) -> Result<Handle, vfs::Error> {
        self.path_to_handle.get(path).cloned().ok_or(vfs::Error::StaleFile)
    }

    /// Returns the parent handle for the given path.
    fn get_parent_handle(&self, path: &Path) -> Result<Handle, vfs::Error> {
        if self.root.join(path) == self.root {
            return Ok(HandleMap::root());
        }
        let parent = path.parent().ok_or(vfs::Error::StaleFile)?;
        self.get_handle_by_path(parent)
    }

    /// Allocates a new handle ID.
    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Creates a new handle for the given path.
    fn create_handle(&mut self, path: &Path) -> Result<Handle, vfs::Error> {
        let parent_path = path.parent().ok_or(vfs::Error::ServerFault)?;
        let name = path
            .file_name()
            .ok_or(vfs::Error::ServerFault)?
            .to_os_string()
            .into_string()
            .map_err(|_| vfs::Error::ServerFault)?;
        let name = Name::new(name).map_err(|_| vfs::Error::ServerFault)?;
        let parent_handle =
            self.path_to_handle.get(parent_path).ok_or(vfs::Error::StaleFile)?.clone();

        let handle = file::Handle(self.alloc_id().to_be_bytes());
        let entry = Entry::new(handle.clone(), path.to_path_buf());

        self.handle_to_path.insert(handle.clone(), entry);
        self.path_to_handle.insert(path.to_path_buf(), handle.clone());

        if let Some(parent_entry) = self.handle_to_path.get_mut(&parent_handle) {
            parent_entry.children.insert(name, handle.clone());
        }

        Ok(handle)
    }

    /// Recursively deletes the entry specified by `path` and `handle`,
    /// including all of its descendant entries from the handle and path maps.
    fn remove_entry_recursive(&mut self, path: &Path, handle: &Handle) -> Result<(), vfs::Error> {
        self.path_to_handle.remove(path).ok_or(vfs::Error::StaleFile)?;
        let entry = self.handle_to_path.remove(handle).ok_or(vfs::Error::StaleFile)?;
        let children: Vec<(file::Name, Handle)> = entry.children.into_iter().collect();

        for (name, child_handle) in children {
            let mut child_path = path.to_path_buf();
            child_path.push(name.as_str());

            let _ = self.remove_entry_recursive(child_path.as_path(), &child_handle);
        }

        Ok(())
    }

    /// Recursively removes the file or directory at the specified `path`,
    /// including all of its descendant entries from the handle and path maps.
    fn remove_path_by_path(&mut self, path: &Path) -> Result<(), vfs::Error> {
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

    /// Removes the entry at `path` by name.
    fn remove_path(
        &mut self,
        path: &Path,
        parent: &Handle,
        name: &file::Name,
    ) -> Result<(), vfs::Error> {
        let parent_entry = self.handle_to_path.get_mut(parent).ok_or(vfs::Error::StaleFile)?;
        let child_handle = parent_entry.children.remove(name).ok_or(vfs::Error::StaleFile)?;
        self.remove_entry_recursive(path, &child_handle)
    }

    /// Creates a new subtree at `new_path` by walking the source subtree
    /// rooted at `old_handle`. Existing destination nodes are removed
    /// best-effort before each new node is created.
    fn rename_entry_recursive(
        &mut self,
        old_path: &Path,
        new_path: &Path,
        old_handle: &Handle,
    ) -> Result<Handle, vfs::Error> {
        let children: Vec<(file::Name, Handle)> = {
            let old_entry = self.handle_to_path.get(old_handle).ok_or(vfs::Error::StaleFile)?;
            old_entry.children.iter().map(|(n, h)| (n.clone(), h.clone())).collect()
        };

        let _ = self.remove_path_by_path(new_path);

        // Parent is guaranteed to exist because we walk top-to-bottom.
        let handle = self.create_handle(new_path)?;
        for (name, child_handle) in children {
            let mut old_child = old_path.to_path_buf();
            let mut new_child = new_path.to_path_buf();
            old_child.push(name.as_str());
            new_child.push(name.as_str());
            self.rename_entry_recursive(old_child.as_path(), new_child.as_path(), &child_handle)?;
        }

        Ok(handle)
    }

    /// Collects all paths in the subtree rooted at `handle` into `acc`.
    fn collect_paths_recursive(
        &self,
        acc: &mut Vec<PathBuf>,
        handle: &Handle,
    ) -> Result<(), vfs::Error> {
        let entry = self.handle_to_path.get(handle).ok_or(vfs::Error::StaleFile)?;
        acc.push(entry.path.clone());
        for child_handle in entry.children.values() {
            self.collect_paths_recursive(acc, child_handle)?;
        }
        Ok(())
    }
}

/// Maintains a bidirectional mapping between NFS file handles and their corresponding
/// relative filesystem paths, along with a directory-to-children index for efficient lookup.
pub struct HandleMap {
    /// The inner state of the handle map.
    inner: RwLock<Inner>,
}

impl HandleMap {
    /// Creates a new handle map rooted at the given absolute path.
    ///
    /// # Parameters
    ///
    /// - `root`: The absolute path to the root directory.
    ///
    /// # Returns
    ///
    /// Returns a new [`HandleMap`] instance.
    pub fn new(root: PathBuf) -> Self {
        let root_handle = file::Handle(ROOT.to_be_bytes());
        let root_relative = PathBuf::new();

        let mut handle_to_path = HashMap::new();
        handle_to_path
            .insert(root_handle.clone(), Entry::new(root_handle.clone(), root_relative.clone()));

        let mut path_to_handle = HashMap::new();
        path_to_handle.insert(root_relative, root_handle);

        Self {
            inner: RwLock::new(Inner { root, handle_to_path, path_to_handle, next_id: ROOT + 1 }),
        }
    }

    /// Returns the fixed handle representing the root directory.
    pub fn root() -> file::Handle {
        file::Handle(ROOT.to_be_bytes())
    }

    /// Returns the stored relative path for a handle.
    ///
    /// # Parameters
    ///
    /// - `handle`: The handle to get the path for.
    ///
    /// # Returns
    ///
    /// - The stored relative path for the given handle.
    ///
    /// # Errors
    ///
    /// Returns [`vfs::Error::StaleFile`] if the handle is not found.
    pub fn path_for_handle(&self, handle: &file::Handle) -> Result<PathBuf, vfs::Error> {
        let inner = self.inner.read().unwrap();

        Ok(inner.handle_to_path.get(handle).ok_or(vfs::Error::StaleFile)?.path.clone())
    }

    /// Returns the stored relative path for a direct child entry.\
    ///
    /// # Parameters
    ///
    /// - `parent`: The parent handle to get the child path for.
    /// - `name`: The name of the child to get the path for.
    ///
    /// # Returns
    ///
    /// - The stored relative path for the given child.
    ///
    /// # Errors
    ///
    /// Returns [`vfs::Error::StaleFile`] if the parent or child is not found.
    pub fn path_for_child(
        &self,
        parent: &file::Handle,
        name: &file::Name,
    ) -> Result<PathBuf, vfs::Error> {
        let inner = self.inner.read().unwrap();

        let parent_entry = inner.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        let child_handle = parent_entry.children.get(name).ok_or(vfs::Error::StaleFile)?;

        Ok(inner.handle_to_path.get(child_handle).ok_or(vfs::Error::StaleFile)?.path.clone())
    }

    /// Returns the direct child handle for a parent and entry name.
    ///
    /// # Parameters
    ///
    /// - `parent`: The parent handle to get the child handle for.
    /// - `name`: The name of the child to get the handle for.
    ///
    /// # Returns
    ///
    /// - The stored handle for the given child.
    ///
    /// # Errors
    ///
    /// Returns [`vfs::Error::StaleFile`] if the parent or child is not found.
    pub fn handle_for_child(
        &self,
        parent: &file::Handle,
        name: &file::Name,
    ) -> Result<Handle, vfs::Error> {
        let inner = self.inner.read().unwrap();
        let parent_entry = inner.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        parent_entry.children.get(name).cloned().ok_or(vfs::Error::StaleFile)
    }

    /// Returns the direct child handle or creates a new direct child entry.
    ///
    /// # Parameters
    ///
    /// - `parent_path`: The path to the parent directory.
    /// - `parent`: The parent handle to create the child for.
    /// - `name`: The name of the child to create.
    ///
    /// # Returns
    ///
    /// - The created handle for the given child.
    ///
    /// # Errors
    ///
    /// Returns [`vfs::Error::StaleFile`] if the parent is not found.
    pub fn ensure_child_handle(
        &self,
        parent_path: &Path,
        parent: &Handle,
        name: &file::Name,
    ) -> Result<Handle, vfs::Error> {
        {
            let inner = self.inner.read().unwrap();
            let parent_entry = inner.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
            if let Some(child_handle) = parent_entry.children.get(name) {
                return Ok(child_handle.clone());
            }
        }

        let mut inner = self.inner.write().unwrap();

        let parent_entry = inner.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        if let Some(child_handle) = parent_entry.children.get(name) {
            return Ok(child_handle.clone());
        }

        let handle = file::Handle(inner.alloc_id().to_be_bytes());
        let mut new_path = parent_path.to_owned();
        new_path.push(name.as_str());
        let entry = Entry::new(handle.clone(), new_path.clone());

        inner.handle_to_path.insert(handle.clone(), entry);
        inner.path_to_handle.insert(new_path, handle.clone());

        let parent_entry = inner.handle_to_path.get_mut(parent).ok_or(vfs::Error::StaleFile)?;
        parent_entry.children.insert(name.clone(), handle.clone());

        Ok(handle)
    }

    /// Removes a direct child from `parent` by name and recursively deletes
    /// the subtree rooted at `path`.
    pub fn remove_path(
        &self,
        path: &Path,
        parent: &Handle,
        name: &file::Name,
    ) -> Result<(), vfs::Error> {
        let mut inner = self.inner.write().unwrap();
        inner.remove_path(path, parent, name)
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
    /// # Parameters
    ///
    /// - `from_parent`: The parent handle of the source entry.
    /// - `from_path`: The path of the source entry.
    /// - `to_path`: The path of the destination entry.
    /// - `from_name`: The name of the source entry.
    /// - `to_name`: The name of the destination entry.
    ///
    /// # Returns
    ///
    /// - The handle of the renamed entry.
    ///
    /// # Errors
    ///
    /// Returns [`vfs::Error::StaleFile`] if the source or destination is not found.
    pub fn rename_path(
        &self,
        from_parent: &Handle,
        from_path: &Path,
        to_path: &Path,
        from_name: &file::Name,
        to_name: &file::Name,
    ) -> Result<Handle, vfs::Error> {
        if from_path == to_path && from_name == to_name {
            return self.handle_for_child(from_parent, from_name);
        }

        let mut inner = self.inner.write().unwrap();

        let parent_entry =
            inner.handle_to_path.get_mut(from_parent).ok_or(vfs::Error::StaleFile)?;
        let source_handle = parent_entry.children.remove(from_name).ok_or(vfs::Error::StaleFile)?;

        let mut old_path = from_path.to_path_buf();
        old_path.push(from_name.as_str());

        let mut new_path = to_path.to_path_buf();
        new_path.push(to_name.as_str());

        let handle =
            inner.rename_entry_recursive(old_path.as_path(), new_path.as_path(), &source_handle)?;
        inner.remove_entry_recursive(old_path.as_path(), &source_handle)?;

        Ok(handle)
    }

    /// Collects relative paths for the subtree rooted at `handle`, including
    /// the root node itself.
    ///
    /// # Parameters
    ///
    /// - `handle`: The handle to collect the paths for.
    ///
    /// # Returns
    ///
    /// A vector of relative paths for the subtree rooted at the given handle.
    pub fn collect_paths_from_subtree(&self, handle: &Handle) -> Result<Vec<PathBuf>, vfs::Error> {
        let inner = self.inner.read().unwrap();
        let mut acc = Vec::new();
        inner.collect_paths_recursive(&mut acc, handle)?;
        Ok(acc)
    }

    /// Converts a relative path into an absolute path under the configured root.
    pub fn to_full_path(&self, relative: &Path) -> Result<PathBuf, vfs::Error> {
        let inner = self.inner.read().unwrap();
        Ok(inner.root.join(relative))
    }
}

/// Validates that a filename is allowed for NFS operations.
///
/// `"."` is rejected as `InvalidArgument`,
/// `".."` is rejected as `Exist`,
/// all other names are accepted.
///
/// # Parameters
///
/// - `name`: The name to check.
///
/// # Returns
///
/// Returns `Ok(())` if the name is allowed.
///
/// # Errors
///
/// Returns the following errors:
///
/// - [`vfs::Error::InvalidArgument`] if the name is `"."`.
/// - [`vfs::Error::Exist`] if the name is `".."`.
pub fn ensure_name_allowed(name: &file::Name) -> Result<(), vfs::Error> {
    match name.as_str() {
        "." => Err(vfs::Error::InvalidArgument),
        ".." => Err(vfs::Error::Exist),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::array::from_ref;
    use std::path::{Path, PathBuf};

    use super::*;

    fn setup() -> HandleMap {
        HandleMap::new(PathBuf::from("/tmp"))
    }

    fn child_name(name: &str) -> file::Name {
        file::Name::new(name.to_string()).unwrap()
    }

    /// Asserts that the tree stored in HandleMap matches the expected state.
    fn assert_state(map: &HandleMap, exp: &[(Handle, PathBuf, &[Handle])]) {
        let inner = map.inner.read().unwrap();

        let mut actual: Vec<(Handle, PathBuf)> = inner
            .handle_to_path
            .iter()
            .map(|(handle, entry)| (handle.clone(), entry.path.clone()))
            .collect();
        actual.sort_by(|(left, _), (right, _)| left.cmp(right));

        let mut expected: Vec<(Handle, PathBuf)> =
            exp.iter().map(|(handle, path, _)| (handle.clone(), path.clone())).collect();
        expected.sort_by(|(left, _), (right, _)| left.cmp(right));

        assert_eq!(actual, expected);

        let mut actual_paths: Vec<(PathBuf, Handle)> = inner
            .path_to_handle
            .iter()
            .map(|(path, handle)| (path.clone(), handle.clone()))
            .collect();
        actual_paths.sort_by(|(left, _), (right, _)| left.cmp(right));

        let mut expected_paths: Vec<(PathBuf, Handle)> =
            exp.iter().map(|(handle, path, _)| (path.clone(), handle.clone())).collect();
        expected_paths.sort_by(|(left, _), (right, _)| left.cmp(right));

        assert_eq!(actual_paths, expected_paths);

        for (handle, _, children) in exp {
            let entry = inner
                .handle_to_path
                .get(handle)
                .unwrap_or_else(|| panic!("missing handle {handle:?}"));

            let mut actual_children: Vec<Handle> = entry.children.values().cloned().collect();
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
        assert_eq!(map.path_for_handle(&h_e).unwrap(), Path::new("a/d/e"));

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
        assert_eq!(map.path_for_handle(&h3).unwrap(), Path::new("x/3"));

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
        assert_eq!(map.path_for_child(&h_dir, &name_b).unwrap(), Path::new("dir/b"));

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
        let _h1_child = map.ensure_child_handle(Path::new("p/a"), &h1, &name_x).unwrap();
        let h2 = map.ensure_child_handle(Path::new("p"), &h_p, &name_b).unwrap();

        assert_eq!(map.handle_for_child(&h_p, &name_a).unwrap(), h1);

        map.remove_path(Path::new("p/a"), &h_p, &name_a).unwrap();

        assert!(map.handle_for_child(&h_p, &name_a).is_err());
        assert!(map.path_for_child(&h_p, &name_a).is_err());
        assert!(map.path_for_handle(&h1).is_err());
        assert!(map.handle_for_child(&h1, &name_x).is_err());
        assert!(map.path_for_handle(&_h1_child).is_err());

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
        assert_eq!(map.path_for_child(&h_p, &name_z).unwrap(), Path::new("p/z"));
        assert_ne!(renamed_child, h1_child);
        assert_eq!(map.path_for_handle(&renamed).unwrap(), Path::new("p/z"));
        assert_eq!(map.path_for_handle(&renamed_child).unwrap(), Path::new("p/z/x"));
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
        assert_eq!(map.path_for_handle(&renamed).unwrap(), Path::new("p/z"));
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
        assert_eq!(map.path_for_child(&h_p, &name).unwrap(), Path::new("p/a"));

        map.remove_path(Path::new("p/a"), &h_p, &name).unwrap();

        assert!(map.handle_for_child(&h_p, &name).is_err());
        assert!(map.path_for_handle(&h_a).is_err());
    }
}
