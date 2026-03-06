use std::collections::HashMap;
use std::path::{Path, PathBuf};

use nfs_mamont::vfs;
use nfs_mamont::vfs::file;

/// Maps mirror paths to opaque VFS handles.
#[derive(Debug)]
pub struct FsMap {
    root: PathBuf,
    next_id: u64,
    id_to_relative: HashMap<u64, PathBuf>,
    relative_to_id: HashMap<PathBuf, u64>,
}

impl FsMap {
    pub fn new(root: PathBuf) -> Self {
        let mut id_to_relative = HashMap::new();
        let mut relative_to_id = HashMap::new();
        id_to_relative.insert(1, PathBuf::new());
        relative_to_id.insert(PathBuf::new(), 1);
        Self { root, next_id: 2, id_to_relative, relative_to_id }
    }

    pub fn root_handle(&self) -> file::Handle {
        Self::encode_handle(1)
    }

    pub fn path_for_handle(&self, handle: &file::Handle) -> Result<PathBuf, vfs::Error> {
        let id = Self::decode_handle(handle)?;
        let relative = self.id_to_relative.get(&id).ok_or(vfs::Error::StaleFile)?;
        Ok(self.to_full_path(relative))
    }

    pub fn ensure_handle_for_path(&mut self, path: &Path) -> Result<file::Handle, vfs::Error> {
        let relative = path
            .strip_prefix(&self.root)
            .map_err(|_| vfs::Error::BadFileHandle)?
            .to_path_buf();

        if let Some(id) = self.relative_to_id.get(&relative).copied() {
            return Ok(Self::encode_handle(id));
        }

        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.id_to_relative.insert(id, relative.clone());
        self.relative_to_id.insert(relative, id);
        Ok(Self::encode_handle(id))
    }

    pub fn remove_path(&mut self, path: &Path) {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return;
        };
        let relative = relative.to_path_buf();
        let mut to_remove = Vec::new();
        for (id, known_relative) in &self.id_to_relative {
            if known_relative == &relative || known_relative.starts_with(&relative) {
                to_remove.push((*id, known_relative.clone()));
            }
        }
        for (id, known_relative) in to_remove {
            self.id_to_relative.remove(&id);
            self.relative_to_id.remove(&known_relative);
        }
    }

    pub fn rename_path(&mut self, from: &Path, to: &Path) -> Result<(), vfs::Error> {
        let from_relative = from
            .strip_prefix(&self.root)
            .map_err(|_| vfs::Error::BadFileHandle)?
            .to_path_buf();
        let to_relative = to
            .strip_prefix(&self.root)
            .map_err(|_| vfs::Error::BadFileHandle)?
            .to_path_buf();

        let updates = self
            .id_to_relative
            .iter()
            .filter_map(|(id, known_relative)| {
                if known_relative == &from_relative || known_relative.starts_with(&from_relative) {
                    let suffix = known_relative.strip_prefix(&from_relative).ok()?.to_path_buf();
                    let mut replacement = to_relative.clone();
                    if !suffix.as_os_str().is_empty() {
                        replacement.push(suffix);
                    }
                    Some((*id, known_relative.clone(), replacement))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for (id, old_relative, new_relative) in updates {
            self.id_to_relative.insert(id, new_relative.clone());
            self.relative_to_id.remove(&old_relative);
            self.relative_to_id.insert(new_relative, id);
        }

        Ok(())
    }

    fn to_full_path(&self, relative: &Path) -> PathBuf {
        if relative.as_os_str().is_empty() {
            self.root.clone()
        } else {
            self.root.join(relative)
        }
    }

    fn encode_handle(id: u64) -> file::Handle {
        file::Handle(id.to_be_bytes())
    }

    fn decode_handle(handle: &file::Handle) -> Result<u64, vfs::Error> {
        let id = u64::from_be_bytes(handle.0);
        if id == 0 {
            Err(vfs::Error::BadFileHandle)
        } else {
            Ok(id)
        }
    }
}
