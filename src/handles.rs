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
