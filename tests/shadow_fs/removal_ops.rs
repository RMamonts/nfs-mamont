use super::common::{empty_attr, file_name, Fixture};
use nfs_mamont::vfs::{self, Vfs as _, WriteMode};

#[tokio::test]
async fn remove_file_cleans_disk_and_state() {
    let fixture = Fixture::new();
    let root = fixture.root();

    let created = fixture
        .fs
        .create(&root, &file_name("temp.txt"), vfs::CreateMode::Unchecked { attr: empty_attr() })
        .await
        .unwrap();

    fixture.fs.write(&created.handle, 0, b"data", WriteMode::FileSync).await.unwrap();

    fixture.fs.remove(&root, &file_name("temp.txt")).await.expect("remove succeeds");

    assert!(!fixture.path("temp.txt").exists());
    let err = fixture.fs.read(&created.handle, 0, 4).await.expect_err("handle becomes stale");
    assert_eq!(err, vfs::NfsError::Stale);
}

#[tokio::test]
async fn remove_dir_deletes_directory() {
    let fixture = Fixture::new();
    let root = fixture.root();

    let _dir = fixture.fs.make_dir(&root, &file_name("sub"), empty_attr()).await.unwrap();

    fixture.fs.remove_dir(&root, &file_name("sub")).await.expect("remove dir succeeds");

    assert!(!fixture.path("sub").exists());
}

#[tokio::test]
async fn remove_rejects_directories() {
    let fixture = Fixture::new();
    let root = fixture.root();

    fixture.fs.make_dir(&root, &file_name("dir"), empty_attr()).await.unwrap();

    let err =
        fixture.fs.remove(&root, &file_name("dir")).await.expect_err("remove directory fails");
    assert_eq!(err, vfs::NfsError::IsDir);
}
