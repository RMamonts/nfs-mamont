use super::common::Fixture;
use nfs_mamont::vfs::{self, DirectoryCookie, FileType, Vfs as _};

#[tokio::test]
async fn read_dir_lists_entries_sorted() {
    let fixture = Fixture::new();
    fixture.write_file("b.txt", b"b");
    fixture.write_file("a.txt", b"a");
    fixture.create_dir("dir");

    let listing = fixture
        .fs
        .read_dir(&fixture.root(), DirectoryCookie(0), vfs::CookieVerifier([0; 8]), 4096)
        .await
        .expect("readdir succeeds");

    let names: Vec<String> = listing.entries.iter().map(|entry| entry.name.0.clone()).collect();
    assert_eq!(names, ["a.txt", "b.txt", "dir"]);
}

#[tokio::test]
async fn read_dir_resumes_with_cookie_and_verifier() {
    let fixture = Fixture::new();
    for name in ["alpha", "beta", "gamma", "delta"] {
        fixture.write_file(name, name.as_bytes());
    }

    let first_batch = fixture
        .fs
        .read_dir(&fixture.root(), DirectoryCookie(0), vfs::CookieVerifier([0; 8]), 128)
        .await
        .expect("first readdir");
    assert!(first_batch.entries.len() < 4);
    let last_cookie = first_batch.entries.last().unwrap().cookie;
    let verifier = first_batch.cookie_verifier;

    let second_batch = fixture
        .fs
        .read_dir(&fixture.root(), last_cookie, verifier, 128)
        .await
        .expect("resume readdir");

    assert!(second_batch.entries.iter().all(|entry| entry.cookie.0 > last_cookie.0));
}

#[tokio::test]
async fn read_dir_plus_returns_handles_and_attrs() {
    let fixture = Fixture::new();
    fixture.write_file("file.txt", b"contents");
    fixture.create_dir("subdir");

    let result = fixture
        .fs
        .read_dir_plus(&fixture.root(), DirectoryCookie(0), vfs::CookieVerifier([0; 8]), 4096, 16)
        .await
        .expect("readdirplus");

    assert!(!result.entries.is_empty());
    for entry in result.entries {
        assert!(entry.handle.is_some());
        assert!(entry.attr.is_some());
        if entry.name.0 == "subdir" {
            assert_eq!(entry.attr.unwrap().file_type, FileType::Directory);
        }
    }
}
