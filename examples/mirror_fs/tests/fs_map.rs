use crate::fs_map::FsMap;
use std::fs;

use nfs_mamont::vfs;
use nfs_mamont::vfs::file;

use super::helpers::expect_err;

#[test]
fn root_handle_and_reused_handles() {
    let tempdir = tempfile::tempdir().unwrap();
    let mut fs_map = FsMap::new(tempdir.path().to_path_buf());

    let root_handle = fs_map.root_handle();
    assert_eq!(fs_map.path_for_handle(&root_handle).unwrap(), tempdir.path());

    let child = tempdir.path().join("nested/file.txt");
    let first = fs_map.ensure_handle_for_path(&child).unwrap();
    let second = fs_map.ensure_handle_for_path(&child).unwrap();
    assert!(first == second);
    assert_eq!(fs_map.path_for_handle(&first).unwrap(), child);

    let outside = tempdir.path().parent().unwrap().join("outside.txt");
    let error = expect_err(fs_map.ensure_handle_for_path(&outside), "outside path must fail");
    assert_eq!(error, vfs::Error::BadFileHandle);
}

#[test]
fn remove_path_invalidates_subtree_handles() {
    let tempdir = tempfile::tempdir().unwrap();
    let mut fs_map = FsMap::new(tempdir.path().to_path_buf());

    let dir = tempdir.path().join("dir");
    let child = dir.join("child.txt");
    let dir_handle = fs_map.ensure_handle_for_path(&dir).unwrap();
    let child_handle = fs_map.ensure_handle_for_path(&child).unwrap();

    fs_map.remove_path(&dir);

    assert_eq!(fs_map.path_for_handle(&dir_handle).unwrap_err(), vfs::Error::StaleFile);
    assert_eq!(fs_map.path_for_handle(&child_handle).unwrap_err(), vfs::Error::StaleFile);
}

#[test]
fn rename_path_moves_cached_descendants() {
    let tempdir = tempfile::tempdir().unwrap();
    let mut fs_map = FsMap::new(tempdir.path().to_path_buf());

    let from = tempdir.path().join("dir");
    let child_from = from.join("child.txt");
    let to = tempdir.path().join("moved");
    let child_to = to.join("child.txt");

    let dir_handle = fs_map.ensure_handle_for_path(&from).unwrap();
    let child_handle = fs_map.ensure_handle_for_path(&child_from).unwrap();

    fs_map.rename_path(&from, &to).unwrap();

    assert_eq!(fs_map.path_for_handle(&dir_handle).unwrap(), to);
    assert_eq!(fs_map.path_for_handle(&child_handle).unwrap(), child_to);
}

#[test]
fn decode_handle_zero_returns_bad_file_handle() {
    let tempdir = tempfile::tempdir().unwrap();
    let fs_map = FsMap::new(tempdir.path().to_path_buf());

    let zero_handle = file::Handle([0u8; 8]);
    assert_eq!(fs_map.path_for_handle(&zero_handle).unwrap_err(), vfs::Error::BadFileHandle);
}

#[test]
fn hard_links_share_same_handle() {
    let tempdir = tempfile::tempdir().unwrap();
    let original = tempdir.path().join("original.txt");
    let alias = tempdir.path().join("alias.txt");
    fs::write(&original, b"hello").unwrap();
    fs::hard_link(&original, &alias).unwrap();

    let mut fs_map = FsMap::new(tempdir.path().to_path_buf());
    let original_handle = fs_map.ensure_handle_for_path(&original).unwrap();
    let alias_handle = fs_map.ensure_handle_for_path(&alias).unwrap();

    assert!(original_handle == alias_handle);
}

#[test]
fn removing_one_hard_link_keeps_handle_alive_if_another_path_is_cached() {
    let tempdir = tempfile::tempdir().unwrap();
    let original = tempdir.path().join("original.txt");
    let alias = tempdir.path().join("alias.txt");
    fs::write(&original, b"hello").unwrap();
    fs::hard_link(&original, &alias).unwrap();

    let mut fs_map = FsMap::new(tempdir.path().to_path_buf());
    let handle = fs_map.ensure_handle_for_path(&original).unwrap();
    let alias_handle = fs_map.ensure_handle_for_path(&alias).unwrap();
    assert!(handle == alias_handle);

    fs_map.remove_path(&alias);

    assert_eq!(fs_map.path_for_handle(&handle).unwrap(), original);
}
