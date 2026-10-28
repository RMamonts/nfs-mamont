use std::io;

use crate::vfs::file::{Name, Path};
use crate::vfs::{MAX_NAME_LEN, MAX_PATH_LEN};

#[test]
fn path_new_rejects_too_long() {
    let input = "a".repeat(MAX_PATH_LEN + 1);
    let err = Path::new(input).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn path_new_accepts_valid_length() {
    let path = Path::new("/tmp/file".to_string()).unwrap();
    assert_eq!(path.into_inner(), std::path::PathBuf::from("/tmp/file"));
}

#[test]
fn path_empty() {
    let err = Path::new(String::new()).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn name_new_accepts_valid_length() {
    let name = Name::new("valid".to_string()).unwrap();
    assert_eq!(name.as_str(), "valid");
}

#[test]
fn name_backslash_error() {
    let err = Name::new("/folder/file.rs".to_string()).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn name_new_rejects_too_long() {
    let input = "a".repeat(MAX_NAME_LEN + 1);
    let err = Name::new(input).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn name_empty() {
    let err = Name::new(String::new()).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}
