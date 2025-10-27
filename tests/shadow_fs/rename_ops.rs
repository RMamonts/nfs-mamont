use super::common::{empty_attr, file_name, Fixture};
use nfs_mamont::vfs::{Vfs as _, WriteMode};
use tokio::fs;

#[tokio::test]
async fn rename_moves_file_between_directories() {
    let fixture = Fixture::new();
    let root = fixture.root();

    let dir = fixture
        .fs
        .make_dir(&root, &file_name("dest"), empty_attr())
        .await
        .expect("create dir")
        .handle;

    let file = fixture
        .fs
        .create(
            &root,
            &file_name("old.txt"),
            nfs_mamont::vfs::CreateMode::Unchecked { attr: empty_attr() },
        )
        .await
        .expect("create file");

    fixture.fs.write(&file.handle, 0, b"renamed", WriteMode::FileSync).await.expect("write file");

    fixture
        .fs
        .rename(&root, &file_name("old.txt"), &dir, &file_name("new.txt"))
        .await
        .expect("rename succeeds");

    assert!(!fixture.path("old.txt").exists());
    assert!(fixture.path("dest/new.txt").exists());

    let read_back = fixture.fs.read(&file.handle, 0, 16).await.expect("read via old handle");
    assert_eq!(read_back.data, b"renamed");

    let disk = fs::read(fixture.path("dest/new.txt")).await.unwrap();
    assert_eq!(disk, b"renamed");
}
