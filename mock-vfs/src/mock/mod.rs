//! [`MockVfs`]: a stateless, deterministic [`Vfs`](nfs_mamont::vfs::Vfs) implementation.
//!
//! The backend keeps no per-object state: every answer is a pure function of the
//! request and of the configuration, so there is nothing to lock or allocate per
//! request and the mock scales with the number of VFS workers. Consequences worth
//! knowing (see [`crate::tree`] for the details):
//!
//! - the directory tree is infinite and self-similar: every directory lists the same
//!   `dir_N` subdirectories and `file_N` files, `LOOKUP` of *any* name succeeds, names
//!   starting with `dir` are directories and every other name is a regular file;
//! - handles are derived from the parent handle and the name, so they are stable
//!   across server restarts and never become stale;
//! - mutations (`WRITE`, `SETATTR`, `CREATE`, `MKDIR`, `REMOVE`, `RMDIR`, `RENAME`)
//!   succeed when they are consistent with the naming rule and are then forgotten;
//! - hard links, symbolic links and special files are not supported.
//!
//! Every procedure first waits for the configured [`Latency`](crate::config::Latency),
//! READ and WRITE additionally pay for the bytes they transfer.

mod attr;
mod data;
mod dir;

#[cfg(test)]
mod tests;

use std::time::{SystemTime, UNIX_EPOCH};

use nfs_mamont::consts::nfsv3::NFS3_WRITEVERFSIZE;
use nfs_mamont::vfs::file::{self, BackendId, Handle};
use nfs_mamont::vfs::{write, WccData};

use crate::config::MockVfsConfig;
use crate::latency::Model;
use crate::tree::{Kind, Layout, ObjectId};

/// `fsid` reported for every object (ASCII `MOCK`).
const FS_ID: u64 = 0x4d4f_434b;
/// Size reported for directories.
const DIR_SIZE: u64 = 4096;

/// Mock backend, see the [module documentation](self).
pub struct MockVfs {
    config: MockVfsConfig,
    model: Model,
    layout: Layout,
    /// Timestamp reported for every object: the moment the backend was created.
    time: file::Time,
    /// WRITE/COMMIT verifier. It changes whenever the backend is recreated, which is
    /// what RFC 1813 expects from a server that lost its unstable writes.
    verifier: [u8; NFS3_WRITEVERFSIZE],
}

impl MockVfs {
    /// Creates a backend serving the tree described by `config`.
    ///
    /// Starts a timer thread unless the configured latency is instant.
    pub fn new(config: MockVfsConfig) -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let time = file::Time {
            seconds: now.as_secs().min(u64::from(u32::MAX)) as u32,
            nanos: now.subsec_nanos(),
        };
        let verifier = (now.as_nanos() as u64).to_be_bytes();
        let layout = Layout { dirs: config.dirs_per_dir, files: config.files_per_dir };
        let model = Model::new(config.latency);

        Self { config, model, layout, time, verifier }
    }

    /// Returns the configuration the backend was created with.
    pub fn config(&self) -> &MockVfsConfig {
        &self.config
    }

    /// Returns the contents every directory is listed with.
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Returns the handle of the root directory as served by backend `backend`.
    ///
    /// `backend` is the index [`nfs_mamont::ServerContext::add_backend`] registered the
    /// mock under; the handle is what the MOUNT service hands out for the export.
    pub fn root_handle(backend: BackendId) -> Handle {
        ObjectId::ROOT.handle(backend)
    }

    /// Returns the attributes of `id`.
    pub fn attr(&self, id: ObjectId) -> file::Attr {
        let (kind, mode, nlink, size) = match id.kind() {
            Kind::Directory => (Kind::Directory, 0o755, 2 + self.layout.dirs as u32, DIR_SIZE),
            Kind::Regular => (Kind::Regular, 0o644, 1, self.config.file_size),
        };

        file::Attr {
            file_type: kind.file_type(),
            mode,
            nlink,
            uid: 0,
            gid: 0,
            size,
            used: size,
            device: file::Device { major: 0, minor: 0 },
            fs_id: FS_ID,
            file_id: id.raw(),
            atime: self.time,
            mtime: self.time,
            ctime: self.time,
        }
    }

    /// Returns the attributes of the object behind `handle`.
    fn attr_of(&self, handle: &Handle) -> file::Attr {
        self.attr(ObjectId::from_handle(handle))
    }

    /// Returns weak cache consistency data of `id`: nothing ever changes, so the
    /// "before" and "after" attributes are the same.
    fn wcc(&self, id: ObjectId) -> WccData {
        let after = self.attr(id);
        let before = file::WccAttr { size: after.size, mtime: after.mtime, ctime: after.ctime };

        WccData { before: Some(before), after: Some(after) }
    }

    fn write_verifier(&self) -> write::Verifier {
        write::Verifier(self.verifier)
    }

    /// Waits for the simulated backend to answer a request transferring `bytes` bytes.
    async fn wait(&self, bytes: u64) {
        self.model.wait(bytes).await;
    }
}
