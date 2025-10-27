use super::common::{file_name, Fixture};
use nfs_mamont::vfs::Vfs as _;
use tokio::fs;

#[tokio::test]
async fn lookup_existing_file_returns_attr() {
    let fixture = Fixture::new();
    fixture.write_file("hello.txt", b"hello world");

    let lookup =
        fixture.fs.lookup(&fixture.root(), &file_name("hello.txt")).await.expect("lookup succeeds");

    assert_eq!(lookup.object_attr.size, 11);
    assert_eq!(lookup.object_attr.file_type, nfs_mamont::vfs::FileType::Regular);
    assert!(lookup.directory_attr.is_some());
}

#[tokio::test]
async fn read_returns_requested_slice() {
    let fixture = Fixture::new();
    fixture.write_file("notes.txt", b"abcdefghijklmnopqrstuvwxyz");

    let handle =
        fixture.fs.lookup(&fixture.root(), &file_name("notes.txt")).await.expect("lookup").handle;

    let read = fixture.fs.read(&handle, 2, 6).await.expect("read succeeds");

    assert_eq!(read.data, b"cdefgh");
    assert_eq!(read.file_attr.unwrap().size, 26);
}

#[tokio::test]
async fn read_zero_count_shortcuts() {
    let fixture = Fixture::new();
    fixture.write_file("empty.bin", b"data");

    let handle = fixture.fs.lookup(&fixture.root(), &file_name("empty.bin")).await.unwrap().handle;

    let read = fixture.fs.read(&handle, 0, 0).await.expect("read");
    assert!(read.data.is_empty());
}

#[tokio::test]
async fn read_past_end_is_empty() {
    let fixture = Fixture::new();
    fixture.write_file("short.txt", b"abc");

    let handle = fixture.fs.lookup(&fixture.root(), &file_name("short.txt")).await.unwrap().handle;

    let read = fixture.fs.read(&handle, 10, 16).await.expect("read past eof");
    assert!(read.data.is_empty());
}

#[tokio::test]
async fn read_link_returns_target() {
    let fixture = Fixture::new();
    // create file and symlink via direct fs operations
    fixture.write_file("file.txt", b"hi");
    std::os::unix::fs::symlink(fixture.path("file.txt"), fixture.path("link")).unwrap();

    let handle = fixture.fs.lookup(&fixture.root(), &file_name("link")).await.unwrap().handle;

    let (target, attr_opt) = fixture.fs.read_link(&handle).await.expect("readlink");
    assert!(target.0.ends_with("file.txt"), "target was {}", target.0);
    assert_eq!(attr_opt.unwrap().file_type, nfs_mamont::vfs::FileType::Symlink);

    // read underlying file through handle, verifying we can follow link using lookup read
    let file_handle =
        fixture.fs.lookup(&fixture.root(), &file_name("file.txt")).await.unwrap().handle;
    let read = fixture.fs.read(&file_handle, 0, 16).await.unwrap();
    assert_eq!(read.data, b"hi");

    // ensure tokio fs still sees same contents for sanity
    let raw = fs::read(fixture.path("file.txt")).await.unwrap();
    assert_eq!(raw, b"hi");
}
