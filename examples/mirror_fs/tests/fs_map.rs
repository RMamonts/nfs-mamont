use crate::fs_map::FsMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use nfs_mamont::vfs;
use nfs_mamont::vfs::file;

use super::helpers::expect_err;

#[tokio::test]
async fn root_handle_and_reused_handles() {
    let tempdir = tempfile::tempdir().unwrap();
    let fs_map = FsMap::new(tempdir.path().to_path_buf());

    let root_handle = fs_map.root_handle();
    assert_eq!(path_for_handle(&fs_map, &root_handle).await.unwrap(), tempdir.path());

    let child = tempdir.path().join("nested/file.txt");
    std::fs::create_dir_all(child.parent().unwrap()).unwrap();
    std::fs::write(&child, b"hello").unwrap();
    let first = ensure_handle_for_path(&fs_map, &child).await.unwrap();
    let second = ensure_handle_for_path(&fs_map, &child).await.unwrap();
    assert!(first == second);
    assert_eq!(path_for_handle(&fs_map, &first).await.unwrap(), child);

    let outside = tempdir.path().parent().unwrap().join("outside.txt");
    std::fs::write(&outside, b"outside").unwrap();
    let error =
        expect_err(ensure_handle_for_path(&fs_map, &outside).await, "outside path must fail");
    assert_eq!(error, vfs::Error::BadFileHandle);
}

#[tokio::test]
async fn remove_path_invalidates_subtree_handles() {
    let tempdir = tempfile::tempdir().unwrap();
    let fs_map = FsMap::new(tempdir.path().to_path_buf());

    let dir = tempdir.path().join("dir");
    let child = dir.join("child.txt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&child, b"hello").unwrap();
    let dir_handle = ensure_handle_for_path(&fs_map, &dir).await.unwrap();
    let child_handle = ensure_handle_for_path(&fs_map, &child).await.unwrap();

    fs_map.remove_path(&dir).await;

    assert_eq!(path_for_handle(&fs_map, &dir_handle).await.unwrap_err(), vfs::Error::StaleFile);
    assert_eq!(path_for_handle(&fs_map, &child_handle).await.unwrap_err(), vfs::Error::StaleFile);
}

#[tokio::test]
async fn rename_path_moves_cached_descendants() {
    let tempdir = tempfile::tempdir().unwrap();
    let fs_map = FsMap::new(tempdir.path().to_path_buf());

    let from = tempdir.path().join("dir");
    let child_from = from.join("child.txt");
    let to = tempdir.path().join("moved");
    let child_to = to.join("child.txt");
    std::fs::create_dir_all(&from).unwrap();
    std::fs::write(&child_from, b"hello").unwrap();

    let dir_handle = ensure_handle_for_path(&fs_map, &from).await.unwrap();
    let child_handle = ensure_handle_for_path(&fs_map, &child_from).await.unwrap();

    fs_map.rename_path(&from, &to).await.unwrap();

    assert_eq!(path_for_handle(&fs_map, &dir_handle).await.unwrap(), to);
    assert_eq!(path_for_handle(&fs_map, &child_handle).await.unwrap(), child_to);
}

#[tokio::test]
async fn decode_handle_zero_returns_bad_file_handle() {
    let tempdir = tempfile::tempdir().unwrap();
    let fs_map = FsMap::new(tempdir.path().to_path_buf());

    let zero_handle = file::Handle([0u8; 8]);
    assert_eq!(
        path_for_handle(&fs_map, &zero_handle).await.unwrap_err(),
        vfs::Error::BadFileHandle
    );
}

#[tokio::test]
async fn hard_links_share_same_handle() {
    let tempdir = tempfile::tempdir().unwrap();
    let original = tempdir.path().join("original.txt");
    let alias = tempdir.path().join("alias.txt");
    fs::write(&original, b"hello").unwrap();
    fs::hard_link(&original, &alias).unwrap();

    let fs_map = FsMap::new(tempdir.path().to_path_buf());
    let original_handle = ensure_handle_for_path(&fs_map, &original).await.unwrap();
    let alias_handle = ensure_handle_for_path(&fs_map, &alias).await.unwrap();

    assert!(original_handle == alias_handle);
}

#[tokio::test]
async fn removing_one_hard_link_keeps_handle_alive_if_another_path_is_cached() {
    let tempdir = tempfile::tempdir().unwrap();
    let original = tempdir.path().join("original.txt");
    let alias = tempdir.path().join("alias.txt");
    fs::write(&original, b"hello").unwrap();
    fs::hard_link(&original, &alias).unwrap();

    let fs_map = FsMap::new(tempdir.path().to_path_buf());
    let handle = ensure_handle_for_path(&fs_map, &original).await.unwrap();
    let alias_handle = ensure_handle_for_path(&fs_map, &alias).await.unwrap();
    assert!(handle == alias_handle);

    fs_map.remove_path(&alias).await;

    assert_eq!(path_for_handle(&fs_map, &handle).await.unwrap(), original);
}

async fn ensure_handle_for_path(fs_map: &FsMap, path: &Path) -> Result<file::Handle, vfs::Error> {
    let attr = attr_for_path(path)?;
    fs_map.ensure_handle_for_attr(path, &attr).await
}

async fn path_for_handle(fs_map: &FsMap, handle: &file::Handle) -> Result<PathBuf, vfs::Error> {
    let candidates = fs_map.path_candidates_for_handle(handle).await?;
    candidates.into_iter().next().ok_or(vfs::Error::StaleFile)
}

fn attr_for_path(path: &Path) -> Result<file::Attr, vfs::Error> {
    let metadata = fs::symlink_metadata(path).map_err(|_| vfs::Error::NoEntry)?;
    let file_type = if metadata.is_dir() { file::Type::Directory } else { file::Type::Regular };
    Ok(file::Attr {
        file_type,
        mode: metadata.mode(),
        nlink: metadata.nlink() as u32,
        uid: metadata.uid(),
        gid: metadata.gid(),
        size: metadata.len(),
        used: metadata.blocks().saturating_mul(512),
        device: file::Device { major: 0, minor: 0 },
        fs_id: metadata.dev(),
        file_id: metadata.ino(),
        atime: file::Time {
            seconds: metadata.atime().max(0) as u32,
            nanos: metadata.atime_nsec().max(0) as u32,
        },
        mtime: file::Time {
            seconds: metadata.mtime().max(0) as u32,
            nanos: metadata.mtime_nsec().max(0) as u32,
        },
        ctime: file::Time {
            seconds: metadata.ctime().max(0) as u32,
            nanos: metadata.ctime_nsec().max(0) as u32,
        },
    })
}
