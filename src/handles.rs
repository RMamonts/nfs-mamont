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
//! # Mutation semantics
//!
//! All multi-step mutations follow a two-phase protocol:
//!
//! 1. **Collection phase**: the operation traverses the relevant
//!    subtrees and records everything it needs. If any lookup fails, the
//!    operation returns an error without modifying state.
//! 2. **Apply phase** (infallible writes): using the data collected in phase 1,
//!    the operation removes old entries and creates new ones. Because every
//!    needed handle and path was validated during collection, this phase
//!    cannot fail.
//!
//! Specific operation notes:
//!
//! - `ensure_child_handle` creates a direct child entry if it does not exist;
//! - removal deletes the target and all its descendants in one batch;
//! - rename creates a mirrored destination subtree, wires it into the
//!   destination parent, then removes the source subtree. Existing destination
//!   nodes are removed before the replacement is created;
//! - rename allocates fresh handles for the destination, so old source handles
//!   become stale after the operation completes.
//!
//! # Path conversion
//!
//! HandleMap stores relative paths only. Conversion to absolute filesystem
//! paths is done via [`HandleMap::to_full_path`] using the configured root.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::vfs;
use crate::vfs::file;
use crate::vfs::file::Handle;

const ROOT: u64 = 1;

/// A directory entry in the handle map.
struct Entry {
    path: PathBuf,
    handle: Handle,
    children: HashMap<file::Name, Handle>,
}

impl Entry {
    fn new(handle: Handle, path: PathBuf) -> Self {
        Self { path, handle, children: HashMap::new() }
    }
}

/// Snapshot of a single node captured during subtree collection.
struct Snapshot {
    path: PathBuf,
    handle: Handle,
    child_names: Vec<file::Name>,
}

/// The inner state of the handle map.
struct Inner {
    root: PathBuf,
    handle_to_path: HashMap<Handle, Entry>,
    path_to_handle: HashMap<PathBuf, Handle>,
    next_id: u64,
}

impl Inner {
    /// Allocates a new handle ID.
    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Iteratively collects the subtree rooted at (`root_path`, `root_handle`).
    ///
    /// Returns nodes in parent-before-children order so that
    /// [`Self::create_mirrored_subtree`] can rely on parents being listed
    /// before their children.
    fn collect_subtree(
        &self,
        root_path: &Path,
        root_handle: &Handle,
    ) -> Result<Vec<Snapshot>, vfs::Error> {
        let mut nodes = Vec::new();
        let mut queue = VecDeque::new();

        queue.push_back((root_path.to_path_buf(), root_handle.clone()));

        while let Some((path, handle)) = queue.pop_front() {
            let entry = self.handle_to_path.get(&handle).ok_or(vfs::Error::StaleFile)?;

            for (name, child_handle) in &entry.children {
                queue.push_back((path.join(name.as_str()), child_handle.clone()));
            }

            let child_names = entry.children.keys().cloned().collect();
            nodes.push(Snapshot { path, handle, child_names });
        }

        Ok(nodes)
    }

    /// Removes all entries listed in `nodes` from both maps.
    ///
    /// Silently skips entries that are already absent.
    fn purge_entries(&mut self, nodes: &[Snapshot]) {
        for node in nodes {
            self.path_to_handle.remove(&node.path);
            self.handle_to_path.remove(&node.handle);
        }
    }

    /// Creates a new subtree that mirrors the structure described by
    /// `source_nodes`, translating every path from `old_root` to `new_root`
    /// and allocating fresh handles.
    ///
    /// Returns the handle of the new root node.
    fn create_mirrored_subtree(
        &mut self,
        old_root: &Path,
        new_root: &Path,
        source_nodes: &[Snapshot],
    ) -> Handle {
        let mut path_to_new_handle: HashMap<PathBuf, Handle> =
            HashMap::with_capacity(source_nodes.len());

        // Allocate handles and insert entries without children.
        for node in source_nodes {
            let suffix = node.path.strip_prefix(old_root).unwrap_or(Path::new(""));
            let new_path = if suffix.as_os_str().is_empty() {
                new_root.to_path_buf()
            } else {
                new_root.join(suffix)
            };

            let handle = file::Handle(self.alloc_id().to_be_bytes());
            let entry = Entry::new(handle.clone(), new_path.clone());

            self.handle_to_path.insert(handle.clone(), entry);
            self.path_to_handle.insert(new_path, handle.clone());
            path_to_new_handle.insert(node.path.clone(), handle);
        }

        // Wire parent-child relationships using the collected names.
        for node in source_nodes {
            let new_handle = &path_to_new_handle[&node.path];
            let entry = self.handle_to_path.get_mut(new_handle).unwrap();
            for name in &node.child_names {
                let child_old_path = node.path.join(name.as_str());
                let child_new_handle = path_to_new_handle[&child_old_path].clone();
                entry.children.insert(name.clone(), child_new_handle);
            }
        }

        path_to_new_handle[&source_nodes[0].path].clone()
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

    /// Returns the direct child handle, creating a new entry if it does not
    /// already exist.
    ///
    /// Uses double-checked locking: first attempts a shared read lock, falling
    /// back to an exclusive write lock only when the child must be created.
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

    /// Removes a direct child from `parent` by name together with all of its
    /// descendants.
    ///
    /// Function works in transaction-like manner: it first collects the subtree to be removed,
    /// then removes the child from the parent and purges the collected subtree.
    ///
    /// # Parameters
    ///
    /// - `path`: The path to the child to remove.
    /// - `parent`: The parent handle to remove the child from.
    /// - `name`: The name of the child to remove.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the child was removed successfully.
    ///
    /// # Errors
    ///
    /// Returns [`vfs::Error::StaleFile`] if the parent or child is not found.
    pub fn remove_path(
        &self,
        path: &Path,
        parent: &Handle,
        name: &file::Name,
    ) -> Result<(), vfs::Error> {
        let mut inner = self.inner.write().unwrap();

        // Collect the subtree to be removed.
        let child_handle = {
            let parent_entry = inner.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
            parent_entry.children.get(name).cloned().ok_or(vfs::Error::StaleFile)?
        };
        let to_remove = inner.collect_subtree(path, &child_handle)?;

        // Remove the child from the parent and purge the collected subtree.
        if let Some(parent_entry) = inner.handle_to_path.get_mut(parent) {
            parent_entry.children.remove(name);
        }

        inner.purge_entries(&to_remove);

        Ok(())
    }

    /// Renames a subtree by creating a mirrored copy at the destination and
    /// then removing the source.
    ///
    /// If source and destination are identical, returns the existing handle
    /// without modification.
    ///
    /// Function works in transaction-like manner: it first collects the source and destination subtrees,
    /// then removes the existing destination subtree and creates the mirrored subtree at the destination.
    /// Finally, it detaches the source from its parent and purges the source subtree.
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
    /// # Error
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

        let old_full = from_path.join(from_name.as_str());
        let new_full = to_path.join(to_name.as_str());

        let source_handle = {
            let parent_entry =
                inner.handle_to_path.get(from_parent).ok_or(vfs::Error::StaleFile)?;
            parent_entry.children.get(from_name).cloned().ok_or(vfs::Error::StaleFile)?
        };

        let source_nodes = inner.collect_subtree(&old_full, &source_handle)?;

        let dest_nodes = if let Some(dest_handle) = inner.path_to_handle.get(&new_full).cloned() {
            inner.collect_subtree(&new_full, &dest_handle)?
        } else {
            Vec::new()
        };

        let dest_parent =
            inner.path_to_handle.get(to_path).cloned().ok_or(vfs::Error::StaleFile)?;

        // Remove existing destination subtree.
        if !dest_nodes.is_empty() {
            if let Some(parent_entry) = inner.handle_to_path.get_mut(&dest_parent) {
                parent_entry.children.remove(to_name);
            }
            inner.purge_entries(&dest_nodes);
        }

        // Create mirrored subtree at destination.
        let new_root_handle = inner.create_mirrored_subtree(&old_full, &new_full, &source_nodes);

        // Wire new root into destination parent.
        if let Some(parent_entry) = inner.handle_to_path.get_mut(&dest_parent) {
            parent_entry.children.insert(to_name.clone(), new_root_handle.clone());
        }

        // Detach and purge source subtree.
        if let Some(parent_entry) = inner.handle_to_path.get_mut(from_parent) {
            parent_entry.children.remove(from_name);
        }
        inner.purge_entries(&source_nodes);

        Ok(new_root_handle)
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
        let root_path = inner.handle_to_path.get(handle).ok_or(vfs::Error::StaleFile)?.path.clone();
        let nodes = inner.collect_subtree(&root_path, handle)?;

        Ok(nodes.into_iter().map(|n| n.path).collect())
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
