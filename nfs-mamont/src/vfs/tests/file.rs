use crate::vfs::file::{Name, Path};
use crate::vfs::{MAX_NAME_LEN, MAX_PATH_LEN};

#[test]
fn path_new_rejects_too_long() {
    let input = "a".repeat(MAX_PATH_LEN + 1);
    let result = Path::new(input);
    assert!(result.is_err());
}

#[test]
fn path_new_accepts_valid_length() {
    let input = "/tmp/file".to_string();
    let result = Path::new(input.clone()).unwrap();
    let expected = std::path::Path::new(&input);
    assert_eq!(result.as_path().as_os_str(), expected.as_os_str());
}

#[test]
fn path_empty() {
    let input = "".to_string();
    let path = Path::new(input);
    assert!(path.is_err());
}

#[test]
fn name_new_accepts_valid_length() {
    let input = "valid".to_string();
    let name = Name::new(input.clone()).unwrap();
    assert_eq!(name.as_str(), input);
}

#[test]
fn name_backslash_error() {
    let input = "/folder/file.rs".to_string();
    let name = Name::new(input);
    assert!(name.is_err());
}
#[test]
fn name_new_rejects_too_long() {
    let input = "a".repeat(MAX_NAME_LEN + 1);
    let result = Name::new(input);
    assert!(result.is_err());
}

#[test]
fn name_empty() {
    let input = "".to_string();
    let name = Name::new(input);
    assert!(name.is_err());
}
