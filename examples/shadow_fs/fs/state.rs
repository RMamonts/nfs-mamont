use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// File identifier reserved for the root directory entry.
pub const ROOT_ID: u64 = 1;

#[derive(Debug)]
pub struct State {
    next_id: u64,
    entries: HashMap<u64, PathBuf>,
    path_index: HashMap<PathBuf, u64>,
}

impl State {
    /// Create a fresh state table containing only the root entry.
    pub fn new() -> Self {
        let mut entries = HashMap::new();
        let mut path_index = HashMap::new();
        let root_rel = PathBuf::new();
        entries.insert(ROOT_ID, root_rel.clone());
        path_index.insert(root_rel, ROOT_ID);
        Self { next_id: ROOT_ID + 1, entries, path_index }
    }

    /// Look up the relative path associated with a file identifier.
    pub fn rel_path(&self, id: u64) -> Option<PathBuf> {
        self.entries.get(&id).cloned()
    }

    /// Return the identifier for the given path, allocating a new one if needed.
    pub fn ensure_entry(&mut self, rel: PathBuf) -> u64 {
        if let Some(&id) = self.path_index.get(&rel) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.entries.insert(id, rel.clone());
        self.path_index.insert(rel, id);
        id
    }

    /// Drop entries whose paths match the supplied prefix.
    pub fn remove_path(&mut self, prefix: &Path) {
        if prefix.as_os_str().is_empty() {
            return;
        }
        let victims: Vec<(u64, PathBuf)> = self
            .entries
            .iter()
            .filter_map(
                |(id, path)| {
                    if path.starts_with(prefix) {
                        Some((*id, path.clone()))
                    } else {
                        None
                    }
                },
            )
            .collect();
        for (id, path) in victims {
            self.entries.remove(&id);
            self.path_index.remove(&path);
        }
    }

    /// Update the entry for `id` and its descendants to reflect a rename.
    pub fn rename_entry(&mut self, id: u64, new_rel: PathBuf) {
        if new_rel.as_os_str().is_empty() {
            return;
        }
        let old_rel = match self.entries.get(&id).cloned() {
            Some(path) if !path.as_os_str().is_empty() => path,
            _ => return,
        };

        if old_rel == new_rel {
            return;
        }

        self.path_index.remove(&old_rel);
        self.entries.insert(id, new_rel.clone());
        self.path_index.insert(new_rel.clone(), id);

        let mut updates = Vec::new();
        for (&child_id, child_path) in self.entries.iter() {
            if child_id == id {
                continue;
            }
            if let Ok(suffix) = child_path.strip_prefix(&old_rel) {
                if suffix.as_os_str().is_empty() {
                    continue;
                }
                let updated = new_rel.join(suffix);
                updates.push((child_id, updated));
            }
        }

        for (child_id, updated) in updates {
            if let Some(old_path) = self.entries.get(&child_id).cloned() {
                self.path_index.remove(&old_path);
                self.entries.insert(child_id, updated.clone());
                self.path_index.insert(updated, child_id);
            }
        }
    }
}
