use std::collections::HashSet;
use std::fs::DirEntry;
use std::fs::OpenOptions;
use std::path::Path;

use crate::utils;

fn create_file(path: impl AsRef<Path>) {
    OpenOptions::new().create_new(true).read(true).write(true).truncate(true).open(&path).unwrap();
}

fn read_dir(path: impl AsRef<Path>) -> impl Iterator<Item = DirEntry> {
    std::fs::read_dir(&path).unwrap().map(Result::unwrap)
}

/// Various directory IO operations.
/// - create directory
/// - perform read_dir on empty directory
/// - create files inside directory
/// - assert them with readdir
/// - delete directory
pub fn create_read_delete(mount_point: impl AsRef<Path>) {
    let dir_path = utils::join(&[&mount_point, &"test_dir"]);

    std::fs::create_dir(&dir_path).unwrap();

    let mut dir_iter = std::fs::read_dir(&dir_path).unwrap();
    assert!(dir_iter.next().is_none());

    const FILES: &[&str] = &["first_file", "second_file"];
    for &file_name in FILES {
        let file_path = utils::join(&[&dir_path, &file_name]);
        create_file(&file_path);
    }

    let file_names: HashSet<_> = FILES.iter().map(ToOwned::to_owned).collect();
    for entry in read_dir(&dir_path) {
        assert!(file_names.contains(entry.file_name().to_str().unwrap()));
    }

    for &file_name in FILES {
        let file_path = utils::join(&[&dir_path, &file_name]);
        std::fs::remove_file(&file_path).unwrap();
    }

    let mut dir_iter = std::fs::read_dir(&dir_path).unwrap();
    assert!(dir_iter.next().is_none());

    std::fs::remove_dir(&dir_path).unwrap();
}
