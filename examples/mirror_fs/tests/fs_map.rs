use crate::fs_map::FsMap;

use nfs_mamont::vfs;

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
