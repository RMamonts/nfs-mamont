use super::common::{empty_attr, file_name, Fixture};
use nfs_mamont::vfs::{self, Vfs as _, WriteMode};
use tokio::fs;

#[tokio::test]
async fn make_symlink_creates_link_and_read_link_returns_target() {
    let fixture = Fixture::new();
    let root = fixture.root();

    let target = fixture
        .fs
        .create(&root, &file_name("target.txt"), vfs::CreateMode::Unchecked { attr: empty_attr() })
        .await
        .unwrap();
    fixture.fs.write(&target.handle, 0, b"payload", WriteMode::FileSync).await.unwrap();

    let created = fixture
        .fs
        .make_symlink(
            &root,
            &file_name("link"),
            &vfs::SymlinkTarget("target.txt".into()),
            empty_attr(),
        )
        .await
        .expect("make symlink");
    assert_eq!(created.attr.file_type, vfs::FileType::Symlink);
    assert!(fixture.path("link").exists());

    let (target_path, _) = fixture.fs.read_link(&created.handle).await.expect("read link");
    assert_eq!(target_path.0, "target.txt");

    let contents = fs::read_link(fixture.path("link")).await.expect("symlink target");
    assert_eq!(contents, std::path::Path::new("target.txt"));
}

#[tokio::test]
async fn make_symlink_rejects_size_attribute() {
    let fixture = Fixture::new();
    let root = fixture.root();

    let mut attr = empty_attr();
    attr.size = Some(10);

    let err = fixture
        .fs
        .make_symlink(&root, &file_name("bad"), &vfs::SymlinkTarget("target".into()), attr)
        .await
        .expect_err("size attribute not supported");
    assert_eq!(err, vfs::NfsError::NotSupp);
}
