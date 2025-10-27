use std::ffi::OsStr;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use tokio::fs::OpenOptions;
use tokio::task;

use nfs_mamont::vfs;

/// Rough per-entry size used to cap directory listing result sizes.
pub const ENTRY_ESTIMATE_BYTES: u32 = 64;

/// Map a host `io::Error` to the closest NFS error code.
pub fn map_io_error(err: io::Error) -> vfs::NfsError {
    use io::ErrorKind::*;
    match err.kind() {
        NotFound => vfs::NfsError::NoEnt,
        PermissionDenied => vfs::NfsError::Access,
        AlreadyExists => vfs::NfsError::Exist,
        InvalidInput | InvalidData => vfs::NfsError::Inval,
        NotADirectory => vfs::NfsError::NotDir,
        IsADirectory => vfs::NfsError::IsDir,
        ReadOnlyFilesystem => vfs::NfsError::RoFs,
        StorageFull | OutOfMemory => vfs::NfsError::NoSpc,
        _ => vfs::NfsError::Io,
    }
}

/// Validate that a component name is a single, non-empty path segment.
pub fn validate_name_component(name: &OsStr) -> vfs::VfsResult<()> {
    if name.is_empty() {
        return Err(vfs::NfsError::Inval);
    }
    if name.len() > vfs::MAX_NAME_LEN {
        return Err(vfs::NfsError::NameTooLong);
    }
    let mut components = Path::new(name).components();
    match components.next() {
        Some(Component::Normal(_)) => {}
        _ => return Err(vfs::NfsError::Inval),
    }
    if components.next().is_some() {
        return Err(vfs::NfsError::Inval);
    }
    Ok(())
}

/// Join a validated child name to a parent relative path.
pub fn join_child(base: &Path, name: &OsStr) -> vfs::VfsResult<PathBuf> {
    validate_name_component(name)?;
    let mut rel = base.to_path_buf();
    rel.push(name);
    Ok(rel)
}

/// Convert an `OsStr` into the string-backed `vfs::FileName` wrapper.
pub fn file_name_string(name: &OsStr) -> vfs::FileName {
    vfs::FileName(name.to_string_lossy().into_owned())
}

/// Apply supported setattr operations to the file at `path`.
pub async fn apply_setattr(path: &Path, attr: &vfs::SetAttr) -> vfs::VfsResult<()> {
    ensure_supported_attr(attr, true, true)?;

    if let Some(size) = attr.size {
        let file = OpenOptions::new().write(true).open(path).await.map_err(map_io_error)?;
        file.set_len(size).await.map_err(map_io_error)?;
    }

    if let Some(mode) = attr.mode {
        let path = path.to_path_buf();
        task::spawn_blocking(move || {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
        })
        .await
        .map_err(|_| vfs::NfsError::ServerFault)?
        .map_err(map_io_error)?;
    }

    Ok(())
}

/// Ensure attribute updates only contain operations the ShadowFS implementation supports.
pub fn ensure_supported_attr(
    attr: &vfs::SetAttr,
    allow_size: bool,
    allow_mode: bool,
) -> vfs::VfsResult<()> {
    if attr.uid.is_some() || attr.gid.is_some() {
        return Err(vfs::NfsError::NotSupp);
    }
    if !matches!(attr.atime, vfs::SetTime::DontChange)
        || !matches!(attr.mtime, vfs::SetTime::DontChange)
    {
        return Err(vfs::NfsError::NotSupp);
    }
    if attr.size.is_some() && !allow_size {
        return Err(vfs::NfsError::NotSupp);
    }
    if attr.mode.is_some() && !allow_mode {
        return Err(vfs::NfsError::NotSupp);
    }
    Ok(())
}

/// Translate std metadata into the NFS-facing attribute representation.
pub fn metadata_to_attr(meta: &std::fs::Metadata, fileid: u64) -> vfs::FileAttr {
    use std::os::unix::fs::FileTypeExt;
    let file_type = meta.file_type();
    let nfs_type = if file_type.is_dir() {
        vfs::FileType::Directory
    } else if file_type.is_file() {
        vfs::FileType::Regular
    } else if file_type.is_symlink() {
        vfs::FileType::Symlink
    } else if file_type.is_char_device() {
        vfs::FileType::CharacterDevice
    } else if file_type.is_block_device() {
        vfs::FileType::BlockDevice
    } else if file_type.is_fifo() {
        vfs::FileType::Fifo
    } else if file_type.is_socket() {
        vfs::FileType::Socket
    } else {
        vfs::FileType::Regular
    };

    vfs::FileAttr {
        file_type: nfs_type,
        mode: meta.mode(),
        nlink: meta.nlink() as u32,
        uid: meta.uid(),
        gid: meta.gid(),
        size: meta.size(),
        used: meta.blocks().saturating_mul(512),
        device: None,
        fsid: meta.dev(),
        fileid,
        atime: vfs::FileTime { seconds: meta.atime(), nanos: meta.atime_nsec() as u32 },
        mtime: vfs::FileTime { seconds: meta.mtime(), nanos: meta.mtime_nsec() as u32 },
        ctime: vfs::FileTime { seconds: meta.ctime(), nanos: meta.ctime_nsec() as u32 },
    }
}

/// Extract the digest fields used in weak-cache-consistency responses.
pub fn digest_from_attr(attr: &vfs::FileAttr) -> vfs::AttrDigest {
    vfs::AttrDigest { size: attr.size, mtime: attr.mtime, ctime: attr.ctime }
}
