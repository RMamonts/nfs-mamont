// use std::collections::{BTreeSet, HashMap};
// use std::os::unix::fs::MetadataExt;
// use std::path::{Path, PathBuf};
//
// use nfs_mamont::vfs;
// use nfs_mamont::vfs::file;
//
// /// Maps mirror paths to opaque VFS handles.
// #[derive(Debug)]
// pub struct FsMap {
//     root: PathBuf,
//     next_id: u64,
//     id_to_key: HashMap<u64, ObjectKey>,
//     key_to_id: HashMap<ObjectKey, u64>,
//     key_to_paths: HashMap<ObjectKey, BTreeSet<PathBuf>>,
//     relative_to_key: HashMap<PathBuf, ObjectKey>,
// }
//
// #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
// struct ObjectKey {
//     dev: u64,
//     ino: u64,
// }
//

use nfs_mamont::vfs::file;

#[derive(Debug)]
pub struct FsMap;
impl FsMap {
    pub fn root_handle(&self) -> file::Handle {
        Self::encode_handle(1)
    }
    fn encode_handle(id: u64) -> file::Handle {
        file::Handle(id.to_be_bytes())
    }
}
//
//     pub fn path_for_handle(&self, handle: &file::Handle) -> Result<PathBuf, vfs::Error> {
//         let id = Self::decode_handle(handle)?;
//         if id == 1 {
//             return Ok(self.root.clone());
//         }
//
//         let key = self.id_to_key.get(&id).ok_or(vfs::Error::StaleFile)?;
//         let paths = self.key_to_paths.get(key).ok_or(vfs::Error::StaleFile)?;
//
//         // Prefer a live path, but do not mutate mappings here.
//         // During in-flight rename/unlink operations another task may still update
//         // aliases; eager pruning here can invalidate a valid handle and cause EIO.
//         for relative in paths {
//             let full = self.to_full_path(relative);
//             if std::fs::symlink_metadata(&full).is_ok() {
//                 return Ok(full);
//             }
//         }
//
//         Err(vfs::Error::StaleFile)
//     }
//
//     pub fn ensure_handle_for_path(&mut self, path: &Path) -> Result<file::Handle, vfs::Error> {
//         let relative =
//             path.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();
//
//         if relative.as_os_str().is_empty() {
//             return Ok(self.root_handle());
//         }
//
//         let key = Self::object_key_for_path(path)?;
//         if let Some(id) = self.key_to_id.get(&key).copied() {
//             self.key_to_paths.entry(key).or_default().insert(relative.clone());
//             self.relative_to_key.insert(relative, key);
//             return Ok(Self::encode_handle(id));
//         }
//
//         let id = self.next_id;
//         self.next_id = self.next_id.wrapping_add(1);
//         if self.next_id <= 1 {
//             self.next_id = 2;
//         }
//         let mut paths = BTreeSet::new();
//         paths.insert(relative.clone());
//
//         self.id_to_key.insert(id, key);
//         self.key_to_id.insert(key, id);
//         self.key_to_paths.insert(key, paths);
//         self.relative_to_key.insert(relative, key);
//         Ok(Self::encode_handle(id))
//     }
//
//     pub fn remove_path(&mut self, path: &Path) {
//         let Ok(relative) = path.strip_prefix(&self.root) else {
//             return;
//         };
//         let relative = relative.to_path_buf();
//
//         let to_remove = self
//             .relative_to_key
//             .keys()
//             .filter(|known_relative| {
//                 *known_relative == &relative || known_relative.starts_with(&relative)
//             })
//             .cloned()
//             .collect::<Vec<_>>();
//
//         for known_relative in to_remove {
//             let Some(key) = self.relative_to_key.remove(&known_relative) else {
//                 continue;
//             };
//             if let Some(paths) = self.key_to_paths.get_mut(&key) {
//                 paths.remove(&known_relative);
//                 if paths.is_empty() {
//                     self.key_to_paths.remove(&key);
//                     if let Some(id) = self.key_to_id.remove(&key) {
//                         self.id_to_key.remove(&id);
//                     }
//                 }
//             }
//         }
//     }
//
//     pub fn rename_path(&mut self, from: &Path, to: &Path) -> Result<(), vfs::Error> {
//         let from_relative =
//             from.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();
//         let to_relative =
//             to.strip_prefix(&self.root).map_err(|_| vfs::Error::BadFileHandle)?.to_path_buf();
//
//         let updates = self
//             .relative_to_key
//             .iter()
//             .filter_map(|(known_relative, key)| {
//                 if known_relative == &from_relative || known_relative.starts_with(&from_relative) {
//                     let suffix = known_relative.strip_prefix(&from_relative).ok()?.to_path_buf();
//                     let mut replacement = to_relative.clone();
//                     if !suffix.as_os_str().is_empty() {
//                         replacement.push(suffix);
//                     }
//                     Some((known_relative.clone(), *key, replacement))
//                 } else {
//                     None
//                 }
//             })
//             .collect::<Vec<_>>();
//
//         for (old_relative, key, new_relative) in updates {
//             self.relative_to_key.remove(&old_relative);
//             self.relative_to_key.insert(new_relative.clone(), key);
//
//             if let Some(paths) = self.key_to_paths.get_mut(&key) {
//                 paths.remove(&old_relative);
//                 paths.insert(new_relative);
//             }
//         }
//
//         Ok(())
//     }
//
//     fn to_full_path(&self, relative: &Path) -> PathBuf {
//         if relative.as_os_str().is_empty() {
//             self.root.clone()
//         } else {
//             self.root.join(relative)
//         }
//     }
//
//     fn encode_handle(id: u64) -> file::Handle {
//         file::Handle(id.to_be_bytes())
//     }
//
//     fn object_key_for_path(path: &Path) -> Result<ObjectKey, vfs::Error> {
//         let metadata = std::fs::symlink_metadata(path).map_err(Self::map_io_error)?;
//         Ok(ObjectKey { dev: metadata.dev(), ino: metadata.ino() })
//     }
//
//     fn decode_handle(handle: &file::Handle) -> Result<u64, vfs::Error> {
//         let id = u64::from_be_bytes(handle.0);
//         if id == 0 {
//             Err(vfs::Error::BadFileHandle)
//         } else {
//             Ok(id)
//         }
//     }
//
//     fn map_io_error(error: std::io::Error) -> vfs::Error {
//         match error.kind() {
//             std::io::ErrorKind::NotFound => vfs::Error::NoEntry,
//             std::io::ErrorKind::PermissionDenied => vfs::Error::Access,
//             std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
//                 vfs::Error::InvalidArgument
//             }
//             _ => vfs::Error::IO,
//         }
//     }
// }
