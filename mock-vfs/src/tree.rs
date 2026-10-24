//! The virtual directory tree served by the mock.
//!
//! The tree is a pure function of names, so the backend needs no state:
//!
//! - every object is identified by an [`ObjectId`] carried in the file handle payload;
//! - the id of a child is a hash of the parent id and the child name, so handles are
//!   deterministic, stable across restarts and never stale;
//! - the kind of an object is decided by its name alone: names starting with
//!   [`DIR_PREFIX`] are directories, every other name is a regular file;
//! - every directory lists the same entries, described by [`Layout`].

use nfs_mamont::vfs::file::{self, BackendId, Handle};

/// Names starting with this prefix denote directories.
pub const DIR_PREFIX: &str = "dir";
/// Prefix of the regular files listed in every directory.
pub const FILE_PREFIX: &str = "file";

/// Kind of a virtual object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Directory,
    Regular,
}

impl Kind {
    /// Decides the kind of an object from its name.
    pub fn of_name(name: &str) -> Kind {
        if name.starts_with(DIR_PREFIX) {
            Kind::Directory
        } else {
            Kind::Regular
        }
    }

    /// Returns the NFS file type of the kind.
    pub fn file_type(self) -> file::Type {
        match self {
            Kind::Directory => file::Type::Directory,
            Kind::Regular => file::Type::Regular,
        }
    }
}

/// Identifier of a virtual object: the payload of its file handle and its `fileid`.
///
/// The lowest bit carries the [`Kind`] (`0` directory, `1` regular file), the other
/// bits a hash of the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectId(u64);

impl ObjectId {
    /// The root directory.
    pub const ROOT: ObjectId = ObjectId(2);

    /// Returns the kind of the object.
    pub fn kind(self) -> Kind {
        if self.0 & 1 == 0 {
            Kind::Directory
        } else {
            Kind::Regular
        }
    }

    /// Returns the raw id, used as `fileid` in attributes and directory entries.
    pub fn raw(self) -> u64 {
        self.0
    }

    /// Returns the id of the entry `name` of this directory.
    ///
    /// `.` and `..` resolve to the directory itself: the parent of a directory is not
    /// recoverable from its id, and NFS clients resolve `..` on their own anyway.
    pub fn child(self, name: &str) -> ObjectId {
        if name == "." || name == ".." {
            return self;
        }
        let kind = match Kind::of_name(name) {
            Kind::Directory => 0,
            Kind::Regular => 1,
        };
        // Children never collide with the root, whose hash part is 1.
        let hash = (fnv1a(self.0, name) >> 1).max(2);

        ObjectId((hash << 1) | kind)
    }

    /// Reads the id from the payload of a handle.
    pub fn from_handle(handle: &Handle) -> ObjectId {
        ObjectId(u64::from_be_bytes(handle.payload()))
    }

    /// Builds the handle of the object as served by backend `backend`.
    pub fn handle(self, backend: BackendId) -> Handle {
        Handle::new(backend, self.0.to_be_bytes())
    }
}

/// 64-bit FNV-1a over the parent id followed by the name.
fn fnv1a(parent: u64, name: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET;
    for byte in parent.to_be_bytes().iter().chain(name.as_bytes()) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// One entry of a directory listing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Name of the entry.
    pub name: String,
    /// Object the entry points to.
    pub id: ObjectId,
    /// Cookie to pass to `READDIR` to continue *after* this entry.
    pub cookie: u64,
}

/// Contents of every directory: `.`, `..`, `dir_0..dir_{dirs-1}`, `file_0..file_{files-1}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Number of subdirectories.
    pub dirs: usize,
    /// Number of regular files.
    pub files: usize,
}

impl Layout {
    /// Returns the number of entries of a directory, `.` and `..` included.
    pub fn len(&self) -> u64 {
        2 + self.dirs as u64 + self.files as u64
    }

    /// Returns `true` when a directory lists only `.` and `..`.
    pub fn is_empty(&self) -> bool {
        self.dirs == 0 && self.files == 0
    }

    /// Returns the entry at `index` of directory `dir`, if any.
    pub fn entry(&self, dir: ObjectId, index: u64) -> Option<Entry> {
        let name = match index {
            0 => ".".to_string(),
            1 => "..".to_string(),
            _ => {
                let position = index - 2;
                let dirs = self.dirs as u64;
                if position < dirs {
                    format!("{DIR_PREFIX}_{position}")
                } else if position - dirs < self.files as u64 {
                    format!("{FILE_PREFIX}_{}", position - dirs)
                } else {
                    return None;
                }
            }
        };
        let id = dir.child(&name);

        Some(Entry { name, id, cookie: index + 1 })
    }

    /// Iterates over the entries of `dir` starting at `index` (a cookie returned by a
    /// previous listing, or `0` for the beginning).
    pub fn entries(&self, dir: ObjectId, index: u64) -> impl Iterator<Item = Entry> + '_ {
        (index..self.len())
            .map(move |index| self.entry(dir, index))
            .take_while(Option::is_some)
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use nfs_mamont::vfs::file::Handle;

    use super::{Kind, Layout, ObjectId};

    #[test]
    fn kind_is_decided_by_the_name() {
        assert_eq!(Kind::of_name("dir_0"), Kind::Directory);
        assert_eq!(Kind::of_name("directory"), Kind::Directory);
        assert_eq!(Kind::of_name("file_0"), Kind::Regular);
        assert_eq!(Kind::of_name("anything.txt"), Kind::Regular);
    }

    #[test]
    fn root_is_a_directory() {
        assert_eq!(ObjectId::ROOT.kind(), Kind::Directory);
    }

    #[test]
    fn children_are_deterministic_and_distinct() {
        let a = ObjectId::ROOT.child("file_0");
        let b = ObjectId::ROOT.child("file_0");
        let c = ObjectId::ROOT.child("file_1");
        let d = ObjectId::ROOT.child("dir_0").child("file_0");

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
        assert_ne!(a, ObjectId::ROOT);
        assert_eq!(a.kind(), Kind::Regular);
        assert_eq!(ObjectId::ROOT.child("dir_0").kind(), Kind::Directory);
    }

    #[test]
    fn dot_entries_resolve_to_the_directory_itself() {
        let dir = ObjectId::ROOT.child("dir_3");

        assert_eq!(dir.child("."), dir);
        assert_eq!(dir.child(".."), dir);
    }

    #[test]
    fn handle_round_trips_and_keeps_the_backend() {
        let id = ObjectId::ROOT.child("dir_1").child("file_7");
        let handle: Handle = id.handle(5);

        assert_eq!(handle.backend_id(), 5);
        assert_eq!(ObjectId::from_handle(&handle), id);
    }

    #[test]
    fn layout_lists_dots_dirs_then_files() {
        let layout = Layout { dirs: 2, files: 3 };
        let names: Vec<String> =
            layout.entries(ObjectId::ROOT, 0).map(|entry| entry.name).collect();

        assert_eq!(names, [".", "..", "dir_0", "dir_1", "file_0", "file_1", "file_2"]);
        assert_eq!(layout.len(), 7);
    }

    #[test]
    fn cookies_continue_after_the_entry() {
        let layout = Layout { dirs: 1, files: 2 };
        let first = layout.entries(ObjectId::ROOT, 0).next().unwrap();
        assert_eq!(first.name, ".");
        assert_eq!(first.cookie, 1);

        let rest: Vec<String> =
            layout.entries(ObjectId::ROOT, first.cookie).map(|entry| entry.name).collect();
        assert_eq!(rest, ["..", "dir_0", "file_0", "file_1"]);

        assert!(layout.entries(ObjectId::ROOT, layout.len()).next().is_none());
    }

    #[test]
    fn listed_entries_match_lookups() {
        let layout = Layout { dirs: 1, files: 1 };
        let dir = ObjectId::ROOT.child("dir_0");

        for entry in layout.entries(dir, 0) {
            assert_eq!(entry.id, dir.child(&entry.name));
        }
    }
}
