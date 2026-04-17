use crate::vfs;
use crate::vfs::file;
use crate::vfs::file::Handle;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

const ROOT: u64 = 1;

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

#[derive(Clone)]
struct Snapshot {
    path: PathBuf,
    handle: Handle,
    children: Vec<file::Name>,
}

struct Inner {
    root: PathBuf,
    generation: u64,
    handle_to_path: HashMap<Handle, Entry>,
}

impl Inner {
    fn new(root: PathBuf) -> Self {
        let root_handle = file::Handle(ROOT.to_be_bytes());
        let root_relative = PathBuf::new();

        let mut handle_to_path = HashMap::new();
        handle_to_path
            .insert(root_handle.clone(), Entry::new(root_handle.clone(), root_relative.clone()));

        Self { root, generation: ROOT + 1, handle_to_path }
    }

    fn alloc_handle(&mut self) -> Handle {
        let id = self.generation;
        self.generation += 1;
        Handle(id.to_be_bytes())
    }

    fn root() -> file::Handle {
        file::Handle(ROOT.to_be_bytes())
    }

    fn path_for_handle(&self, handle: &file::Handle) -> Result<PathBuf, vfs::Error> {
        Ok(self.handle_to_path.get(handle).ok_or(vfs::Error::StaleFile)?.path.clone())
    }

    fn path_for_child(
        &self,
        parent: &file::Handle,
        name: &file::Name,
    ) -> Result<PathBuf, vfs::Error> {
        let parent_entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        let child_handle = parent_entry.children.get(name).ok_or(vfs::Error::StaleFile)?;
        Ok(self.handle_to_path.get(child_handle).ok_or(vfs::Error::StaleFile)?.path.clone())
    }

    fn handle_for_child(
        &self,
        parent: &file::Handle,
        name: &file::Name,
    ) -> Result<Handle, vfs::Error> {
        let parent_entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
        parent_entry.children.get(name).cloned().ok_or(vfs::Error::StaleFile)
    }

    fn ensure_child_handle(
        &mut self,
        parent: &Handle,
        name: &file::Name,
    ) -> Result<Handle, vfs::Error> {
        {
            let parent_entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
            if let Some(child_handle) = parent_entry.children.get(name) {
                return Ok(child_handle.clone());
            }
        }

        let mut new_path = self.path_for_handle(parent)?;
        new_path.push(name.as_str());
        let handle = self.alloc_handle();

        let entry = Entry::new(handle.clone(), new_path.clone());

        self.handle_to_path.insert(handle.clone(), entry);

        let parent_entry = self.handle_to_path.get_mut(parent).ok_or(vfs::Error::StaleFile)?;
        parent_entry.children.insert(name.clone(), handle.clone());

        Ok(handle)
    }

    fn remove_path(&mut self, parent: &Handle, name: &file::Name) -> Result<(), vfs::Error> {
        let child_handle = {
            let parent_entry = self.handle_to_path.get(parent).ok_or(vfs::Error::StaleFile)?;
            parent_entry.children.get(name).cloned().ok_or(vfs::Error::StaleFile)?
        };

        let path = self.path_for_child(parent, name)?;

        let to_remove = self.collect_subtree(&path, &child_handle)?;

        // Remove the child from the parent and purge the collected subtree.
        if let Some(parent_entry) = self.handle_to_path.get_mut(parent) {
            parent_entry.children.remove(name);
        }

        self.purge_entries(&to_remove);

        Ok(())
    }

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
            nodes.push(Snapshot { path, handle, children: child_names });
        }

        Ok(nodes)
    }

    fn purge_entries(&mut self, nodes: &[Snapshot]) {
        for node in nodes {
            self.handle_to_path.remove(&node.handle);
        }
    }

    fn is_destination_inside_source(source: &Path, destination: &Path) -> bool {
        destination != source && destination.starts_with(source)
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
        let mut path_to_new_handle = HashMap::with_capacity(source_nodes.len());
        let mut old_path_to_new_path = HashMap::with_capacity(source_nodes.len());
        let mut new_path_to_handle = HashMap::with_capacity(source_nodes.len());

        // Allocate handles and insert entries without children.
        for node in source_nodes {
            let (new_path, handle) = if node.path.starts_with(new_root) {
                (node.path.clone(), node.handle.clone())
            } else {
                let suffix = node.path.strip_prefix(old_root).unwrap_or(Path::new(""));
                let mapped = if suffix.as_os_str().is_empty() {
                    new_root.to_path_buf()
                } else {
                    new_root.join(suffix)
                };
                (mapped, self.alloc_handle())
            };

            let entry = Entry::new(handle.clone(), new_path.clone());
            self.handle_to_path.insert(handle.clone(), entry);

            path_to_new_handle.insert(node.path.clone(), handle.clone());
            old_path_to_new_path.insert(node.path.clone(), new_path.clone());
            new_path_to_handle.insert(new_path, handle);
        }

        // Wire parent-child relationships based on final paths, so preserved
        // destination nodes are attached even when their parent is replaced.
        for (old_path, new_path) in &old_path_to_new_path {
            if new_path == new_root {
                continue;
            }

            if let Some(parent_new_path) = new_path.parent() {
                if let Some(parent_handle) = new_path_to_handle.get(parent_new_path) {
                    if let Some(name) = new_path.file_name().and_then(|n| n.to_str()) {
                        let name = file::Name::new(name.to_string()).unwrap();
                        let child_handle = path_to_new_handle[old_path].clone();
                        let entry = self.handle_to_path.get_mut(parent_handle).unwrap();
                        entry.children.insert(name, child_handle);
                    }
                }
            }
        }

        path_to_new_handle[&source_nodes[0].path].clone()
    }

    fn rename_path(
        &mut self,
        from_parent: &Handle,
        from_name: &file::Name,
        to_parent: &Handle,
        to_name: &file::Name,
    ) -> Result<Handle, vfs::Error> {
        if from_parent == to_parent && from_name == to_name {
            return self.handle_for_child(from_parent, from_name);
        }

        let old_full = self.path_for_child(from_parent, from_name)?;
        let new_full = self.path_for_handle(to_parent)?.join(to_name.as_str());
        if Self::is_destination_inside_source(&old_full, &new_full) {
            return Err(vfs::Error::InvalidArgument);
        }

        let source_handle = self.handle_for_child(from_parent, from_name)?;

        let source_nodes = self.collect_subtree(&old_full, &source_handle)?;

        let dest_nodes = if let Ok(dest_handle) = self.handle_for_child(to_parent, to_name) {
            self.collect_subtree(&new_full, &dest_handle)?
        } else {
            Vec::new()
        };

        let old_copy = old_full.clone();
        let dest_subtree_to_preserve = dest_nodes
            .iter()
            .filter(|snap| {
                source_nodes
                    .iter()
                    .map(|source_snap| {
                        new_full
                            .join(source_snap.path.strip_prefix(&old_copy).unwrap_or(Path::new("")))
                    })
                    .all(|x| x != snap.path)
            })
            .cloned()
            .collect::<Vec<Snapshot>>();

        // Remove existing destination subtree.
        if !dest_nodes.is_empty() {
            if let Some(parent_entry) = self.handle_to_path.get_mut(&to_parent) {
                parent_entry.children.remove(to_name);
            }
            self.purge_entries(&dest_nodes);
        }

        // Build the resulting subtree from source plus preserved destination nodes.
        let mut nodes_for_new_tree = source_nodes.clone();
        nodes_for_new_tree.extend(dest_subtree_to_preserve);

        // Create mirrored subtree at destination.
        let new_root_handle =
            self.create_mirrored_subtree(&old_full, &new_full, &nodes_for_new_tree);

        // Wire new root into destination parent.
        if let Some(parent_entry) = self.handle_to_path.get_mut(&to_parent) {
            parent_entry.children.insert(to_name.clone(), new_root_handle.clone());
        }

        // Detach and purge source subtree.
        if let Some(parent_entry) = self.handle_to_path.get_mut(from_parent) {
            parent_entry.children.remove(from_name);
        }
        self.purge_entries(&source_nodes);

        Ok(new_root_handle)
    }
}

struct HandleMap {
    map: RwLock<Inner>,
}

impl HandleMap {
    fn new(root: PathBuf) -> Self {
        Self { map: RwLock::new(Inner::new(root)) }
    }
}

#[cfg(test)]
mod tests {
    use std::array::from_ref;
    use std::path::{Path, PathBuf};

    use super::*;

    fn setup() -> Inner {
        Inner::new(PathBuf::from("/tmp"))
    }

    fn child_name(name: &str) -> file::Name {
        file::Name::new(name.to_string()).unwrap()
    }

    /// Asserts state of `Inner::handle_to_path` and child links.
    fn assert_state(map: &Inner, exp: &[(Handle, PathBuf, &[Handle])]) {
        let mut actual: Vec<(Handle, PathBuf)> = map
            .handle_to_path
            .iter()
            .map(|(handle, entry)| (handle.clone(), entry.path.clone()))
            .collect();
        actual.sort_by(|(left, _), (right, _)| left.cmp(right));

        let mut expected: Vec<(Handle, PathBuf)> =
            exp.iter().map(|(handle, path, _)| (handle.clone(), path.clone())).collect();
        expected.sort_by(|(left, _), (right, _)| left.cmp(right));

        assert_eq!(actual, expected);

        for (handle, _, children) in exp {
            let entry = map
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

    ///          /
    ///          |
    ///          a
    ///       /  |  \
    ///      b   c   d
    ///              |
    ///              e
    ///
    /// Change: repeated create for `a/b` returns the same handle.
    #[test]
    fn test_multiple_insertions() {
        let mut map = setup();

        assert!(matches!(map.path_for_handle(&Handle([9; 8])), Err(vfs::Error::StaleFile)));

        let h_root = Inner::root();
        let name_a = child_name("a");
        let name_b = child_name("b");
        let name_c = child_name("c");
        let name_d = child_name("d");
        let name_e = child_name("e");

        assert!(matches!(map.handle_for_child(&h_root, &name_a), Err(vfs::Error::StaleFile)));
        let h_a = map.ensure_child_handle(&h_root, &name_a).unwrap();
        let h_b = map.ensure_child_handle(&h_a, &name_b).unwrap();
        let h_b2 = map.ensure_child_handle(&h_a, &name_b).unwrap();
        let _h_c = map.ensure_child_handle(&h_a, &name_c).unwrap();
        let h_d = map.ensure_child_handle(&h_a, &name_d).unwrap();
        let h_e = map.ensure_child_handle(&h_d, &name_e).unwrap();

        assert_eq!(h_b, h_b2);
        assert_eq!(map.handle_for_child(&h_a, &name_d).unwrap(), h_d);
        assert_eq!(map.path_for_handle(&h_e).unwrap(), Path::new("a/d/e"));

        let h_c = map.handle_for_child(&h_a, &name_c).unwrap();
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

    ///          /
    ///          |
    ///          x
    ///       /  |  \
    ///      1   2   3
    ///
    /// Change: siblings under `x` are reachable with correct paths.
    #[test]
    fn test_children_population() {
        let mut map = setup();

        let name_x = child_name("x");
        let name_1 = child_name("1");
        let name_2 = child_name("2");
        let name_3 = child_name("3");

        assert!(matches!(
            map.handle_for_child(&Inner::root(), &name_x),
            Err(vfs::Error::StaleFile)
        ));
        let h_x = map.ensure_child_handle(&Inner::root(), &name_x).unwrap();
        let h1 = map.ensure_child_handle(&h_x, &name_1).unwrap();
        let h2 = map.ensure_child_handle(&h_x, &name_2).unwrap();
        let h3 = map.ensure_child_handle(&h_x, &name_3).unwrap();

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
                (Inner::root(), PathBuf::new(), from_ref(&h_x)),
                (h_x.clone(), PathBuf::from("x"), &[h1.clone(), h2.clone(), h3.clone()]),
                (h1, PathBuf::from("x/1"), &[]),
                (h2, PathBuf::from("x/2"), &[]),
                (h3, PathBuf::from("x/3"), &[]),
            ],
        );
    }

    ///          /
    ///          |
    ///         dir
    ///        /   \
    ///       a     b
    ///
    /// Change: second create of `dir/a` reuses existing handle.
    #[test]
    fn test_existing_files() {
        let mut map = setup();

        let name_dir = child_name("dir");
        let name_a = child_name("a");
        let name_b = child_name("b");
        assert!(matches!(
            map.handle_for_child(&Inner::root(), &name_dir),
            Err(vfs::Error::StaleFile)
        ));
        let h_dir = map.ensure_child_handle(&Inner::root(), &name_dir).unwrap();
        let h1 = map.ensure_child_handle(&h_dir, &name_a).unwrap();
        let _h2 = map.ensure_child_handle(&h_dir, &name_b).unwrap();

        assert_eq!(map.handle_for_child(&h_dir, &name_a).unwrap(), h1);
        assert_eq!(map.path_for_child(&h_dir, &name_b).unwrap(), Path::new("dir/b"));

        let h1b = map.ensure_child_handle(&h_dir, &name_a).unwrap();
        assert_eq!(h1, h1b);

        let h2 = map.handle_for_child(&h_dir, &name_b).unwrap();
        assert_state(
            &map,
            &[
                (Inner::root(), PathBuf::new(), from_ref(&h_dir)),
                (h_dir.clone(), PathBuf::from("dir"), &[h1.clone(), h2.clone()]),
                (h1, PathBuf::from("dir/a"), &[]),
                (h2, PathBuf::from("dir/b"), &[]),
            ],
        );
    }

    ///          /
    ///          |
    ///          p
    ///       /     \
    ///      a       b
    ///      |
    ///      x
    ///
    /// Change: remove `p/a`; subtree `a -> x` becomes stale, `p/b` remains.
    #[test]
    fn test_remove_child_removes_subtree() {
        let mut map = setup();

        let name_p = child_name("p");
        let name_a = child_name("a");
        let name_b = child_name("b");
        let name_x = child_name("x");
        let h_p = map.ensure_child_handle(&Inner::root(), &name_p).unwrap();
        let h1 = map.ensure_child_handle(&h_p, &name_a).unwrap();
        let h1_child = map.ensure_child_handle(&h1, &name_x).unwrap();
        let h2 = map.ensure_child_handle(&h_p, &name_b).unwrap();

        assert_eq!(map.handle_for_child(&h_p, &name_a).unwrap(), h1);

        map.remove_path(&h_p, &name_a).unwrap();

        assert!(matches!(map.handle_for_child(&h_p, &name_a), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_child(&h_p, &name_a), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_handle(&h1), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.handle_for_child(&h1, &name_x), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_handle(&h1_child), Err(vfs::Error::StaleFile)));
        assert_eq!(map.handle_for_child(&h_p, &name_b).unwrap(), h2);

        assert_state(
            &map,
            &[
                (Inner::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), from_ref(&h2)),
                (h2.clone(), PathBuf::from("p/b"), &[]),
            ],
        );
    }

    ///          /
    ///          |
    ///          p
    ///       /     \
    ///      a       z
    ///      |       |
    ///      x       q
    ///
    /// Change: rename `p/a -> p/z`; `x` replaces destination root content, `q` is preserved.
    #[test]
    fn test_rename_path_updates_subtree() {
        let mut map = setup();

        let name_p = child_name("p");
        let name_a = child_name("a");
        let name_x = child_name("x");
        let name_z = child_name("z");
        let name_q = child_name("q");
        let h_p = map.ensure_child_handle(&Inner::root(), &name_p).unwrap();
        let h1 = map.ensure_child_handle(&h_p, &name_a).unwrap();
        let h1_child = map.ensure_child_handle(&h1, &name_x).unwrap();
        let h_z = map.ensure_child_handle(&h_p, &name_z).unwrap();
        let h_z_child = map.ensure_child_handle(&h_z, &name_q).unwrap();

        assert_eq!(map.handle_for_child(&h_p, &name_a).unwrap(), h1);

        let renamed = map.rename_path(&h_p, &name_a, &h_p, &name_z).unwrap();
        let renamed_child = map.handle_for_child(&renamed, &name_x).unwrap();

        assert!(matches!(map.handle_for_child(&h_p, &name_a), Err(vfs::Error::StaleFile)));
        assert_eq!(map.handle_for_child(&h_p, &name_z).unwrap(), renamed);
        assert_eq!(map.path_for_child(&h_p, &name_z).unwrap(), Path::new("p/z"));
        assert_ne!(renamed_child, h1_child);
        assert_eq!(map.path_for_handle(&renamed).unwrap(), Path::new("p/z"));
        assert_eq!(map.path_for_handle(&renamed_child).unwrap(), Path::new("p/z/x"));
        assert!(matches!(map.path_for_handle(&h1), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_handle(&h1_child), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_handle(&h_z), Err(vfs::Error::StaleFile)));
        assert_eq!(map.path_for_handle(&h_z_child).unwrap(), Path::new("p/z/q"));
        assert_eq!(map.handle_for_child(&renamed, &name_q).unwrap(), h_z_child);

        let renamed_child = map.handle_for_child(&renamed, &name_x).unwrap();
        assert_state(
            &map,
            &[
                (Inner::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), from_ref(&renamed)),
                (
                    renamed.clone(),
                    PathBuf::from("p/z"),
                    &[renamed_child.clone(), h_z_child.clone()],
                ),
                (renamed_child, PathBuf::from("p/z/x"), &[]),
                (h_z_child, PathBuf::from("p/z/q"), &[]),
            ],
        );
    }

    ///          /
    ///          |
    ///          p
    ///       /     \
    ///      a       z
    ///      |       |
    ///      x       q
    ///
    /// Change: after `p/a -> p/z`, no-op rename `p/z -> p/z` keeps the same root handle.
    #[test]
    fn test_rename_path_replaces_existing_destination() {
        let mut map = setup();

        let name_p = child_name("p");
        let name_a = child_name("a");
        let name_x = child_name("x");
        let name_z = child_name("z");
        let name_q = child_name("q");
        let h_p = map.ensure_child_handle(&Inner::root(), &name_p).unwrap();
        let h_a = map.ensure_child_handle(&h_p, &name_a).unwrap();
        let h_a_child = map.ensure_child_handle(&h_a, &name_x).unwrap();
        let h_z = map.ensure_child_handle(&h_p, &name_z).unwrap();
        let h_z_child = map.ensure_child_handle(&h_z, &name_q).unwrap();

        let renamed = map.rename_path(&h_p, &name_a, &h_p, &name_z).unwrap();
        let renamed_child = map.handle_for_child(&renamed, &name_x).unwrap();

        assert_eq!(map.handle_for_child(&h_p, &name_z).unwrap(), renamed);
        assert_ne!(map.handle_for_child(&renamed, &name_x).unwrap(), h_a_child);

        assert!(matches!(map.handle_for_child(&h_p, &name_a), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_handle(&h_a), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_handle(&h_z), Err(vfs::Error::StaleFile)));
        assert_eq!(map.path_for_handle(&h_z_child).unwrap(), Path::new("p/z/q"));
        assert_eq!(map.handle_for_child(&renamed, &name_q).unwrap(), h_z_child);
        assert_eq!(map.path_for_handle(&renamed).unwrap(), Path::new("p/z"));
        map.path_for_handle(&renamed_child).unwrap();
        assert!(matches!(map.path_for_handle(&h_a_child), Err(vfs::Error::StaleFile)));

        let renamed2 = map.rename_path(&h_p, &name_z, &h_p, &name_z).unwrap();

        assert_eq!(renamed, renamed2);
        assert_state(
            &map,
            &[
                (Inner::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), from_ref(&renamed)),
                (
                    renamed.clone(),
                    PathBuf::from("p/z"),
                    &[renamed_child.clone(), h_z_child.clone()],
                ),
                (renamed_child, PathBuf::from("p/z/x"), &[]),
                (h_z_child, PathBuf::from("p/z/q"), &[]),
            ],
        );
    }

    ///          /
    ///          |
    ///          p
    ///          |
    ///          a
    ///
    /// Change: create/lookup/remove for `p/a`, then verify `a` is stale.
    #[test]
    fn test_parent_child_helpers() {
        let mut map = setup();
        let h_p = map.ensure_child_handle(&Inner::root(), &child_name("p")).unwrap();
        let name = child_name("a");
        let h_a = map.ensure_child_handle(&h_p, &name).unwrap();

        assert_eq!(map.handle_for_child(&h_p, &name).unwrap(), h_a);
        assert_eq!(map.path_for_child(&h_p, &name).unwrap(), Path::new("p/a"));

        map.remove_path(&h_p, &name).unwrap();

        assert!(matches!(map.handle_for_child(&h_p, &name), Err(vfs::Error::StaleFile)));
        assert!(matches!(map.path_for_handle(&h_a), Err(vfs::Error::StaleFile)));
        assert_state(
            &map,
            &[
                (Inner::root(), PathBuf::new(), from_ref(&h_p)),
                (h_p.clone(), PathBuf::from("p"), &[]),
            ],
        );
    }
}
