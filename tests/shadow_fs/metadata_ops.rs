use super::common::{empty_attr, file_name, Fixture};
use nfs_mamont::vfs::{self, AccessMask, SetAttrGuard, Vfs as _};
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn set_attr_updates_size_and_mode() {
    let fixture = Fixture::new();
    let root = fixture.root();

    let created = fixture
        .fs
        .create(&root, &file_name("resize.txt"), vfs::CreateMode::Unchecked { attr: empty_attr() })
        .await
        .unwrap();

    fixture.fs.write(&created.handle, 0, b"abc", vfs::WriteMode::FileSync).await.unwrap();

    let mut attr = empty_attr();
    attr.size = Some(1);
    attr.mode = Some(0o700);

    let wcc = fixture
        .fs
        .set_attr(&created.handle, attr, SetAttrGuard::None)
        .await
        .expect("set attr succeeds");
    assert!(wcc.before.is_some() && wcc.after.is_some());

    let metadata = std::fs::metadata(fixture.path("resize.txt")).unwrap();
    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata.permissions().mode() & 0o777, 0o700);
}

#[tokio::test]
async fn access_returns_mask_for_read_and_execute() {
    let fixture = Fixture::new();
    let root = fixture.root();

    fixture.write_file("script.sh", b"echo hi");
    let handle = fixture.fs.lookup(&root, &file_name("script.sh")).await.unwrap().handle;

    // make file executable
    let mut attr = empty_attr();
    attr.mode = Some(0o755);
    fixture.fs.set_attr(&handle, attr, SetAttrGuard::None).await.unwrap();

    let mut mask = AccessMask::empty();
    mask.insert(AccessMask::READ);
    mask.insert(AccessMask::EXECUTE);
    let result = fixture.fs.access(&handle, mask).await.expect("access succeeds");
    assert!(result.granted.contains(AccessMask::READ));
    assert!(result.granted.contains(AccessMask::EXECUTE));
}

#[tokio::test]
async fn fs_stat_reports_defaults_and_attr() {
    let fixture = Fixture::new();
    let root = fixture.root();

    let stat = fixture.fs.fs_stat(&root).await.expect("fs_stat");
    assert_eq!(stat.total_bytes, 0);
    assert!(stat.file_attr.is_some());

    let info = fixture.fs.fs_info(&root).await.expect("fs_info");
    assert!(info.read_max >= info.read_pref);

    let conf = fixture.fs.path_conf(&root).await.expect("path_conf");
    assert!(conf.max_name >= nfs_mamont::vfs::MAX_NAME_LEN as u32);
}
