use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::fs;
use tokio::sync::Mutex;

use nfs_mamont::vfs;

use super::state::{State, ROOT_ID};
use super::utils::{digest_from_attr, map_io_error, metadata_to_attr};

/// Shadow filesystem wrapper backed by the host filesystem.
#[derive(Debug)]
pub struct ShadowFS {
    root: PathBuf,
    state: Mutex<State>,
    verifier: vfs::StableVerifier,
}

impl ShadowFS {
    /// Instantiate the filesystem, canonicalising the root path and seeding the verifier.
    pub fn new(root: PathBuf) -> Self {
        let canonical = root.canonicalize().unwrap_or(root);
        let verifier_seed =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
        Self {
            root: canonical,
            state: Mutex::new(State::new()),
            verifier: vfs::StableVerifier(verifier_seed.to_le_bytes()),
        }
    }

    /// Borrow the canonical root path.
    #[allow(dead_code)]
    pub fn root_path(&self) -> &Path {
        &self.root
    }

    /// Encode the root directory as a file handle.
    pub fn root_handle(&self) -> vfs::FileHandle {
        Self::encode_handle(ROOT_ID)
    }

    /// Serialise an identifier into an NFS file handle.
    pub fn encode_handle(id: u64) -> vfs::FileHandle {
        vfs::FileHandle(id.to_le_bytes().to_vec())
    }

    /// Recover the identifier stored in a file handle.
    pub fn decode_handle(handle: &vfs::FileHandle) -> vfs::VfsResult<u64> {
        if handle.0.len() != 8 {
            return Err(vfs::NfsError::BadHandle);
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&handle.0);
        Ok(u64::from_le_bytes(buf))
    }

    /// Combine the root with a relative path.
    pub fn full_path(&self, rel: &Path) -> PathBuf {
        if rel.as_os_str().is_empty() {
            self.root.clone()
        } else {
            self.root.join(rel)
        }
    }

    /// Resolve a file handle to its tracked relative path.
    pub async fn rel_path_from_handle(&self, handle: &vfs::FileHandle) -> vfs::VfsResult<PathBuf> {
        let id = Self::decode_handle(handle)?;
        self.rel_path_from_id(id).await
    }

    /// Resolve a file identifier to its relative path.
    pub async fn rel_path_from_id(&self, id: u64) -> vfs::VfsResult<PathBuf> {
        let state = self.state.lock().await;
        state.rel_path(id).ok_or(vfs::NfsError::Stale)
    }

    /// Ensure an entry exists for a path and return its identifier.
    pub async fn ensure_entry(&self, rel: PathBuf) -> u64 {
        let mut state = self.state.lock().await;
        state.ensure_entry(rel)
    }

    /// Remove all entries beneath the given relative path.
    pub async fn remove_entry(&self, rel: &Path) {
        let mut state = self.state.lock().await;
        state.remove_path(rel);
    }

    /// Update an identifier to point to a new relative location.
    pub async fn rename_entry(&self, source_id: u64, new_rel: PathBuf) {
        let mut state = self.state.lock().await;
        state.remove_path(&new_rel);
        state.rename_entry(source_id, new_rel);
    }

    /// Fetch metadata for the supplied relative path.
    pub async fn metadata_for_rel(&self, rel: &Path) -> vfs::VfsResult<std::fs::Metadata> {
        let abs = self.full_path(rel);
        fs::symlink_metadata(&abs).await.map_err(map_io_error)
    }

    /// Fetch metadata for a tracked identifier.
    pub async fn metadata_for_id(&self, id: u64) -> vfs::VfsResult<std::fs::Metadata> {
        let rel = self.rel_path_from_id(id).await?;
        self.metadata_for_rel(&rel).await
    }

    /// Convert metadata into an attribute record.
    pub fn attr_from_meta(id: u64, meta: &std::fs::Metadata) -> vfs::FileAttr {
        metadata_to_attr(meta, id)
    }

    /// Extract the digest portion of an attribute record.
    pub fn digest_from_attr(attr: &vfs::FileAttr) -> vfs::AttrDigest {
        digest_from_attr(attr)
    }

    /// Return the directory cookie verifier.
    pub fn cookie_verifier(&self) -> vfs::CookieVerifier {
        vfs::CookieVerifier(self.verifier.0)
    }

    /// Check a supplied verifier matches the current one.
    pub fn verify_cookie(&self, provided: vfs::CookieVerifier) -> vfs::VfsResult<()> {
        if provided.0 == [0; 8] || provided == self.cookie_verifier() {
            Ok(())
        } else {
            Err(vfs::NfsError::BadCookie)
        }
    }

    /// Expose the current stable write verifier.
    pub fn stable_verifier(&self) -> vfs::StableVerifier {
        self.verifier
    }
}
