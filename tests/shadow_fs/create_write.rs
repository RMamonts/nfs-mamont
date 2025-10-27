use super::common::{empty_attr, file_name, Fixture};
use nfs_mamont::vfs::{self, CreateMode, Vfs as _, WriteMode};
use std::os::unix::fs::PermissionsExt;
use tokio::fs;

#[tokio::test]
async fn create_unchecked_honors_mode() {
    let fixture = Fixture::new();

    let mut attr = empty_attr();
    attr.mode = Some(0o640);

    let created = fixture
        .fs
        .create(&fixture.root(), &file_name("report.txt"), CreateMode::Unchecked { attr })
        .await
        .expect("create succeeds");

    assert_eq!(created.attr.file_type, vfs::FileType::Regular);
    let metadata = std::fs::metadata(fixture.path("report.txt")).expect("stat file");
    assert_eq!(metadata.permissions().mode() & 0o777, 0o640);
}

#[tokio::test]
async fn write_returns_requested_commit_mode() {
    let fixture = Fixture::new();
    let attr = empty_attr();

    let handle = fixture
        .fs
        .create(&fixture.root(), &file_name("data.bin"), CreateMode::Unchecked { attr })
        .await
        .unwrap()
        .handle;

    let payload = vec![1u8; 32];
    let result =
        fixture.fs.write(&handle, 0, &payload, WriteMode::DataSync).await.expect("write succeeds");

    assert_eq!(result.count, payload.len() as u32);
    assert_eq!(result.committed, WriteMode::DataSync);

    let read_back = fs::read(fixture.path("data.bin")).await.expect("read file");
    assert_eq!(read_back, payload);
}

#[tokio::test]
async fn commit_refuses_ranges_beyond_end() {
    let fixture = Fixture::new();
    let attr = empty_attr();

    let created = fixture
        .fs
        .create(&fixture.root(), &file_name("log.txt"), CreateMode::Unchecked { attr })
        .await
        .unwrap();
    let write_res =
        fixture.fs.write(&created.handle, 0, b"abc", WriteMode::FileSync).await.unwrap();

    let err = fixture.fs.commit(&created.handle, 16, 4).await.expect_err("commit fails");
    assert_eq!(err, vfs::NfsError::Inval);

    let ok = fixture.fs.commit(&created.handle, 0, 0).await.expect("full commit succeeds");
    assert_eq!(ok.verifier, write_res.verifier);
}
