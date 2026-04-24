use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use nfs_mamont::consts::nfsv3::NFS3_COOKIEVERFSIZE;
use nfs_mamont::vfs;
use nfs_mamont::vfs::access;
use nfs_mamont::vfs::commit;
use nfs_mamont::vfs::file;
use nfs_mamont::vfs::fs_info;
use nfs_mamont::vfs::fs_stat;
use nfs_mamont::vfs::get_attr;
use nfs_mamont::vfs::path_conf;
use nfs_mamont::vfs::read;
use nfs_mamont::vfs::read_dir;
use nfs_mamont::vfs::read_dir_plus;
use nfs_mamont::vfs::read_link;
use nfs_mamont::vfs::write;
use tokio::time::{sleep, Duration};

use crate::fs::READ_WRITE_MAX;

use super::helpers::{
    alloc_slice, create_dir, create_symlink, expect_err, expect_ok, slice_to_vec, write_file,
    TestContext,
};

#[tokio::test]
async fn access_returns_requested_mask() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "file.txt", b"hello");
    let root = ctx.root_handle().await;
    let handle = ctx.lookup_handle(root, "file.txt").await;

    let result = expect_ok(
        access::Access::access(
            &ctx.fs,
            access::Args {
                file: handle,
                mask: access::Mask::from_wire(access::Mask::READ | access::Mask::MODIFY),
            },
        )
        .await,
        "access should succeed",
    );

    assert_eq!(result.access.bits(), access::Mask::READ | access::Mask::MODIFY);
    assert!(matches!(result.object_attr.unwrap().file_type, file::Type::Regular));
}

#[tokio::test]
async fn access_respects_file_permissions() {
    let ctx = TestContext::new();
    let path = write_file(ctx.root_path(), "readonly.txt", b"data");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    let root = ctx.root_handle().await;
    let handle = ctx.lookup_handle(root, "readonly.txt").await;

    let result = expect_ok(
        access::Access::access(
            &ctx.fs,
            access::Args {
                file: handle,
                mask: access::Mask::from_wire(
                    access::Mask::READ | access::Mask::MODIFY | access::Mask::EXECUTE,
                ),
            },
        )
        .await,
        "access should succeed on read-only file",
    );

    assert!(result.access.contains(access::Mask::READ));
    assert!(!result.access.contains(access::Mask::MODIFY));
    assert!(!result.access.contains(access::Mask::EXECUTE));
}

#[tokio::test]
async fn commit_flushes_regular_file_and_rejects_directory() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "file.txt", b"hello");
    create_dir(ctx.root_path(), "dir");
    let root = ctx.root_handle().await;
    let file_handle = ctx.lookup_handle(root.clone(), "file.txt").await;
    let dir_handle = ctx.lookup_handle(root, "dir").await;

    let success = expect_ok(
        commit::Commit::commit(&ctx.fs, commit::Args { file: file_handle, offset: 0, count: 0 })
            .await,
        "commit should succeed for regular files",
    );
    super::helpers::assert_wcc_present(&success.file_wcc);
    assert_eq!(success.verifier.0.len(), 8);

    let fail = expect_err(
        commit::Commit::commit(&ctx.fs, commit::Args { file: dir_handle, offset: 0, count: 0 })
            .await,
        "commit should fail for directories",
    );
    assert_eq!(fail.error, vfs::Error::InvalidArgument);
    super::helpers::assert_wcc_present(&fail.file_wcc);
}

#[tokio::test]
async fn fs_info_returns_server_limits() {
    let ctx = TestContext::new();
    let root = ctx.root_handle().await;

    let result = expect_ok(
        fs_info::FsInfo::fs_info(&ctx.fs, fs_info::Args { root }).await,
        "fs_info should succeed",
    );
    let properties = result.properties.bits();

    assert!(result.root_attr.is_some());
    assert_eq!(result.read_max, READ_WRITE_MAX);
    assert_eq!(result.write_max, READ_WRITE_MAX);
    assert_eq!(result.read_dir_pref, 8 * 1024);
    assert!(properties & fs_info::Properties::LINK != 0);
    assert!(properties & fs_info::Properties::SYMLINK != 0);
    assert!(properties & fs_info::Properties::CANSETTIME != 0);
}

#[tokio::test]
async fn fs_stat_returns_zero_counters() {
    let ctx = TestContext::new();
    let root = ctx.root_handle().await;

    let result = expect_ok(
        fs_stat::FsStat::fs_stat(&ctx.fs, fs_stat::Args { root }).await,
        "fs_stat should succeed",
    );

    assert!(result.root_attr.is_some());
    assert_eq!(result.total_bytes, 0);
    assert_eq!(result.free_bytes, 0);
    assert_eq!(result.available_files, 0);
    assert_eq!(result.invarsec, 0);
}

#[tokio::test]
async fn get_attr_returns_metadata() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "file.txt", b"hello");
    let root = ctx.root_handle().await;
    let handle = ctx.lookup_handle(root, "file.txt").await;

    let result = expect_ok(
        get_attr::GetAttr::get_attr(&ctx.fs, get_attr::Args { file: handle }).await,
        "get_attr should succeed",
    );

    assert!(matches!(result.object.file_type, file::Type::Regular));
    assert_eq!(result.object.size, 5);
}

#[tokio::test]
async fn path_conf_reports_limits() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "file.txt", b"hello");
    let root = ctx.root_handle().await;
    let handle = ctx.lookup_handle(root, "file.txt").await;

    let result = expect_ok(
        path_conf::PathConf::path_conf(&ctx.fs, path_conf::Args { file: handle }).await,
        "path_conf should succeed",
    );

    assert!(result.file_attr.is_some());
    assert_eq!(result.link_max, u32::MAX);
    assert_eq!(result.name_max, vfs::MAX_NAME_LEN as u32);
    assert!(result.no_trunc);
    assert!(result.chown_restricted);
    assert!(!result.case_insensitive);
    assert!(result.case_preserving);
}

#[tokio::test]
async fn read_reads_requested_window_and_rejects_directories() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "file.txt", b"abcdef");
    create_dir(ctx.root_path(), "dir");
    let root = ctx.root_handle().await;
    let file_handle = ctx.lookup_handle(root.clone(), "file.txt").await;
    let dir_handle = ctx.lookup_handle(root, "dir").await;

    let success = expect_ok(
        read::Read::read(
            &ctx.fs,
            read::Args { file: file_handle, offset: 2, count: 3 },
            alloc_slice(3).await,
        )
        .await,
        "read should succeed",
    );
    assert_eq!(success.head.count, 3);
    assert!(!success.head.eof);
    assert_eq!(slice_to_vec(&success.data), b"cde");

    let eof = expect_ok(
        read::Read::read(
            &ctx.fs,
            read::Args {
                file: ctx.lookup_handle(ctx.root_handle().await, "file.txt").await,
                offset: 99,
                count: 5,
            },
            alloc_slice(5).await,
        )
        .await,
        "read past eof should succeed",
    );
    assert_eq!(eof.head.count, 0);
    assert!(eof.head.eof);

    let fail = expect_err(
        read::Read::read(
            &ctx.fs,
            read::Args { file: dir_handle, offset: 0, count: 1 },
            alloc_slice(1).await,
        )
        .await,
        "read on directory should fail",
    );
    assert_eq!(fail.error, vfs::Error::InvalidArgument);
}

#[tokio::test]
async fn write_invalidates_prefetched_read_ahead_blocks() {
    let ctx = TestContext::new();
    let mut payload = vec![b'a'; READ_WRITE_MAX as usize * 2];
    payload[READ_WRITE_MAX as usize..].fill(b'b');
    write_file(ctx.root_path(), "large.bin", &payload);
    let cached_path = ctx.root_path().join("large.bin");
    let root = ctx.root_handle().await;
    let handle = ctx.lookup_handle(root, "large.bin").await;

    let warmup = expect_ok(
        read::Read::read(
            &ctx.fs,
            read::Args { file: handle.clone(), offset: 0, count: 512 * 1024 },
            alloc_slice(512 * 1024).await,
        )
        .await,
        "warmup read should succeed",
    );
    assert_eq!(warmup.head.count, 512 * 1024);

    for _ in 0..50 {
        if ctx.fs.cached_read_ahead_blocks_for(&cached_path) > 0 {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    assert!(ctx.fs.cached_read_ahead_blocks_for(&cached_path) > 0);

    let write_result = expect_ok(
        write::Write::write(
            &ctx.fs,
            write::Args {
                file: handle.clone(),
                offset: READ_WRITE_MAX as u64,
                size: 1,
                stable: write::StableHow::FileSync,
                data: super::helpers::slice_from_bytes(b"Z").await,
            },
        )
        .await,
        "write after warmup should succeed",
    );
    assert_eq!(write_result.count, 1);
    assert_eq!(ctx.fs.cached_read_ahead_blocks_for(&cached_path), 0);

    let refreshed = expect_ok(
        read::Read::read(
            &ctx.fs,
            read::Args { file: handle, offset: READ_WRITE_MAX as u64, count: 1 },
            alloc_slice(1).await,
        )
        .await,
        "read after invalidation should succeed",
    );
    assert_eq!(slice_to_vec(&refreshed.data), b"Z");
}

#[tokio::test]
async fn repeated_reads_keep_stable_cached_shard_affinity() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "stable.bin", b"abcdefgh");
    let cached_path = ctx.root_path().join("stable.bin");
    let handle = ctx.lookup_handle(ctx.root_handle().await, "stable.bin").await;

    expect_ok(
        read::Read::read(
            &ctx.fs,
            read::Args { file: handle.clone(), offset: 0, count: 4 },
            alloc_slice(4).await,
        )
        .await,
        "first read should succeed",
    );
    let first_shard = ctx
        .fs
        .cached_read_file_shard_for(&cached_path)
        .expect("cached shard should exist after first read");

    expect_ok(
        read::Read::read(
            &ctx.fs,
            read::Args { file: handle, offset: 4, count: 4 },
            alloc_slice(4).await,
        )
        .await,
        "second read should succeed",
    );
    let second_shard = ctx
        .fs
        .cached_read_file_shard_for(&cached_path)
        .expect("cached shard should exist after second read");

    assert_eq!(first_shard, second_shard);
}

#[tokio::test]
async fn read_dir_returns_sorted_entries_and_rejects_bad_cookie() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "b.txt", b"b");
    write_file(ctx.root_path(), "a.txt", b"a");
    write_file(ctx.root_path(), "c.txt", b"c");
    let root = ctx.root_handle().await;

    let success = expect_ok(
        read_dir::ReadDir::read_dir(
            &ctx.fs,
            read_dir::Args {
                dir: root.clone(),
                cookie: read_dir::Cookie::new(0),
                cookie_verifier: read_dir::CookieVerifier::new([0; NFS3_COOKIEVERFSIZE]),
                count: 4096,
            },
        )
        .await,
        "read_dir should succeed",
    );
    let names =
        success.entries.iter().map(|entry| entry.file_name.as_str().to_owned()).collect::<Vec<_>>();
    assert_eq!(names, vec!["a.txt", "b.txt", "c.txt"]);

    let fail = expect_err(
        read_dir::ReadDir::read_dir(
            &ctx.fs,
            read_dir::Args {
                dir: root,
                cookie: read_dir::Cookie::new(1),
                cookie_verifier: read_dir::CookieVerifier::new([1; NFS3_COOKIEVERFSIZE]),
                count: 4096,
            },
        )
        .await,
        "read_dir should reject bad cookies",
    );
    assert_eq!(fail.error, vfs::Error::BadCookie);
}

#[tokio::test]
async fn read_dir_plus_returns_handles_and_supports_pagination() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "a.txt", b"a");
    write_file(ctx.root_path(), "b.txt", b"b");
    write_file(ctx.root_path(), "c.txt", b"c");
    let root = ctx.root_handle().await;

    let first = expect_ok(
        read_dir_plus::ReadDirPlus::read_dir_plus(
            &ctx.fs,
            read_dir_plus::Args {
                dir: root.clone(),
                cookie: read_dir::Cookie::new(0),
                cookie_verifier: read_dir::CookieVerifier::new([0; NFS3_COOKIEVERFSIZE]),
                dir_count: 0,
                max_count: 130,
            },
        )
        .await,
        "read_dir_plus first page should succeed",
    );
    assert_eq!(first.entries.len(), 2);
    assert!(!first.eof);
    assert!(first
        .entries
        .iter()
        .all(|entry| entry.file_attr.is_some() && entry.file_handle.is_some()));

    let second = expect_ok(
        read_dir_plus::ReadDirPlus::read_dir_plus(
            &ctx.fs,
            read_dir_plus::Args {
                dir: root,
                cookie: first.entries.last().unwrap().cookie,
                cookie_verifier: first.cookie_verifier,
                dir_count: 0,
                max_count: 4096,
            },
        )
        .await,
        "read_dir_plus second page should succeed",
    );
    assert_eq!(second.entries.len(), 1);
    assert!(second.eof);
    assert_eq!(second.entries[0].file_name.as_str(), "c.txt");
}

#[tokio::test]
async fn read_link_returns_target_and_rejects_regular_files() {
    let ctx = TestContext::new();
    create_symlink(ctx.root_path(), "target.txt", "link.txt");
    write_file(ctx.root_path(), "file.txt", b"hello");
    let root = ctx.root_handle().await;
    let link_handle = ctx.lookup_handle(root.clone(), "link.txt").await;
    let file_handle = ctx.lookup_handle(root, "file.txt").await;

    let success = expect_ok(
        read_link::ReadLink::read_link(&ctx.fs, read_link::Args { file: link_handle }).await,
        "read_link should succeed",
    );
    assert!(matches!(success.symlink_attr.unwrap().file_type, file::Type::Symlink));
    assert_eq!(success.data.as_path(), Path::new("target.txt"));

    let fail = expect_err(
        read_link::ReadLink::read_link(&ctx.fs, read_link::Args { file: file_handle }).await,
        "read_link should fail for regular files",
    );
    assert_eq!(fail.error, vfs::Error::InvalidArgument);
}
