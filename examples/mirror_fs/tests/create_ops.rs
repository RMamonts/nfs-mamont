use std::fs as stdfs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;

use nfs_mamont::consts::nfsv3::NFS3_CREATEVERFSIZE;
use nfs_mamont::vfs;
use nfs_mamont::vfs::commit;
use nfs_mamont::vfs::create;
use nfs_mamont::vfs::file;
use nfs_mamont::vfs::get_attr;
use nfs_mamont::vfs::link;
use nfs_mamont::vfs::mk_dir;
use nfs_mamont::vfs::mk_node;
use nfs_mamont::vfs::read;
use nfs_mamont::vfs::remove;
use nfs_mamont::vfs::set_attr;
use nfs_mamont::vfs::symlink;
use nfs_mamont::vfs::write;
use nfs_mamont::Slice;

use super::helpers::{
    alloc_slice, assert_wcc_present, create_dir, default_new_attr, dir_op, expect_err, expect_ok,
    file_path, sized_attr, slice_from_bytes, slice_to_vec, write_file, TestContext,
};

#[tokio::test]
async fn create_supports_unchecked_guarded_and_exclusive() {
    let ctx = TestContext::new();
    let root = ctx.root_handle().await;

    let unchecked = expect_ok(
        create::Create::create(
            &ctx.fs,
            create::Args {
                object: dir_op(root.clone(), "alpha.txt"),
                how: create::How::Unchecked(sized_attr(Some(0o640), Some(5))),
            },
        )
        .await,
        "unchecked create should succeed",
    );
    assert!(unchecked.file.is_some());
    assert!(matches!(unchecked.attr.unwrap().file_type, file::Type::Regular));
    assert_wcc_present(&unchecked.wcc_data);
    let alpha_meta = stdfs::metadata(ctx.root_path().join("alpha.txt")).unwrap();
    assert_eq!(alpha_meta.len(), 5);
    assert_eq!(alpha_meta.permissions().mode() & 0o777, 0o640);

    let unchecked_existing = expect_ok(
        create::Create::create(
            &ctx.fs,
            create::Args {
                object: dir_op(root.clone(), "alpha.txt"),
                how: create::How::Unchecked(sized_attr(Some(0o600), Some(2))),
            },
        )
        .await,
        "unchecked create should update existing file",
    );
    assert!(unchecked_existing.file.is_some());
    let alpha_meta = stdfs::metadata(ctx.root_path().join("alpha.txt")).unwrap();
    assert_eq!(alpha_meta.len(), 2);
    assert_eq!(alpha_meta.permissions().mode() & 0o777, 0o600);

    let guarded_fail = expect_err(
        create::Create::create(
            &ctx.fs,
            create::Args {
                object: dir_op(root.clone(), "alpha.txt"),
                how: create::How::Guarded(default_new_attr()),
            },
        )
        .await,
        "guarded create should fail for existing file",
    );
    assert_eq!(guarded_fail.error, vfs::Error::Exist);
    assert_wcc_present(&guarded_fail.wcc_data);

    let exclusive = expect_ok(
        create::Create::create(
            &ctx.fs,
            create::Args {
                object: dir_op(root.clone(), "beta.txt"),
                how: create::How::Exclusive(create::Verifier([7u8; NFS3_CREATEVERFSIZE])),
            },
        )
        .await,
        "exclusive create should succeed",
    );
    assert!(exclusive.file.is_some());
    assert_eq!(stdfs::metadata(ctx.root_path().join("beta.txt")).unwrap().len(), 0);

    // Idempotent retry with the SAME verifier should succeed (RFC 1813 §3.3.8)
    let retry = expect_ok(
        create::Create::create(
            &ctx.fs,
            create::Args {
                object: dir_op(root.clone(), "beta.txt"),
                how: create::How::Exclusive(create::Verifier([7u8; NFS3_CREATEVERFSIZE])),
            },
        )
        .await,
        "exclusive create retry with same verifier should succeed",
    );
    assert!(retry.file.is_some());

    // Retry with a DIFFERENT verifier should fail with Exist
    let different = expect_err(
        create::Create::create(
            &ctx.fs,
            create::Args {
                object: dir_op(root, "beta.txt"),
                how: create::How::Exclusive(create::Verifier([99u8; NFS3_CREATEVERFSIZE])),
            },
        )
        .await,
        "exclusive create retry with different verifier should fail",
    );
    assert_eq!(different.error, vfs::Error::Exist);
}

#[tokio::test]
async fn link_creates_hard_link_and_rejects_directory() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "original.txt", b"hello");
    create_dir(ctx.root_path(), "dir");
    let root = ctx.root_handle().await;
    let original = ctx.lookup_handle(root.clone(), "original.txt").await;
    let dir = ctx.lookup_handle(root.clone(), "dir").await;

    let success = expect_ok(
        link::Link::link(
            &ctx.fs,
            link::Args { file: original, link: dir_op(root.clone(), "alias.txt") },
        )
        .await,
        "hard link should succeed",
    );
    assert!(success.file_attr.is_some());
    assert_wcc_present(&success.dir_wcc);

    let original_meta = stdfs::metadata(ctx.root_path().join("original.txt")).unwrap();
    let alias_meta = stdfs::metadata(ctx.root_path().join("alias.txt")).unwrap();
    assert_eq!(original_meta.ino(), alias_meta.ino());
    assert_eq!(original_meta.nlink(), 2);

    let fail = expect_err(
        link::Link::link(&ctx.fs, link::Args { file: dir, link: dir_op(root, "dir-link") }).await,
        "linking directories should fail",
    );
    assert_eq!(fail.error, vfs::Error::InvalidArgument);
}

#[tokio::test]
async fn mk_dir_creates_directory_and_applies_mode() {
    let ctx = TestContext::new();
    let root = ctx.root_handle().await;

    let success = expect_ok(
        mk_dir::MkDir::mk_dir(
            &ctx.fs,
            mk_dir::Args {
                object: dir_op(root.clone(), "child"),
                attr: sized_attr(Some(0o750), None),
            },
        )
        .await,
        "mk_dir should succeed",
    );
    assert!(success.file.is_some());
    assert!(matches!(success.attr.unwrap().file_type, file::Type::Directory));
    assert_wcc_present(&success.wcc_data);
    let meta = stdfs::metadata(ctx.root_path().join("child")).unwrap();
    assert!(meta.is_dir());
    assert_eq!(meta.permissions().mode() & 0o777, 0o750);
}

#[tokio::test]
async fn mk_node_handles_supported_and_unsupported_types() {
    let ctx = TestContext::new();
    let root = ctx.root_handle().await;

    let regular = expect_ok(
        mk_node::MkNode::mk_node(
            &ctx.fs,
            mk_node::Args {
                object: dir_op(root.clone(), "node-file.txt"),
                what: mk_node::What::Regular,
            },
        )
        .await,
        "mk_node regular should succeed",
    );
    assert!(regular.file.is_some());
    assert!(matches!(regular.attr.unwrap().file_type, file::Type::Regular));

    let directory = expect_ok(
        mk_node::MkNode::mk_node(
            &ctx.fs,
            mk_node::Args {
                object: dir_op(root.clone(), "node-dir"),
                what: mk_node::What::Directory,
            },
        )
        .await,
        "mk_node directory should succeed",
    );
    assert!(directory.file.is_some());
    assert!(matches!(directory.attr.unwrap().file_type, file::Type::Directory));

    let bad_type = expect_err(
        mk_node::MkNode::mk_node(
            &ctx.fs,
            mk_node::Args {
                object: dir_op(root.clone(), "node-link"),
                what: mk_node::What::SymbolicLink,
            },
        )
        .await,
        "mk_node symlink should fail with bad type",
    );
    assert_eq!(bad_type.error, vfs::Error::BadType);

    let not_supported = expect_err(
        mk_node::MkNode::mk_node(
            &ctx.fs,
            mk_node::Args {
                object: dir_op(root, "node-socket"),
                what: mk_node::What::Socket(default_new_attr()),
            },
        )
        .await,
        "mk_node socket should be unsupported",
    );
    assert_eq!(not_supported.error, vfs::Error::NotSupported);
}

#[tokio::test]
async fn set_attr_updates_size_and_honors_guard() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "file.txt", b"hello");
    let root = ctx.root_handle().await;
    let handle = ctx.lookup_handle(root, "file.txt").await;
    let current = expect_ok(
        get_attr::GetAttr::get_attr(&ctx.fs, get_attr::Args { file: handle.clone() }).await,
        "get_attr should succeed before set_attr",
    );

    let success = expect_ok(
        set_attr::SetAttr::set_attr(
            &ctx.fs,
            set_attr::Args {
                file: handle.clone(),
                new_attr: sized_attr(Some(0o600), Some(2)),
                guard: Some(set_attr::Guard { ctime: current.object.ctime }),
            },
        )
        .await,
        "set_attr should succeed with matching guard",
    );
    assert_wcc_present(&success.wcc_data);
    let meta = stdfs::metadata(ctx.root_path().join("file.txt")).unwrap();
    assert_eq!(meta.len(), 2);
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);

    let wrong_guard = file::Time {
        seconds: current.object.ctime.seconds.saturating_add(1),
        nanos: current.object.ctime.nanos,
    };
    let fail = expect_err(
        set_attr::SetAttr::set_attr(
            &ctx.fs,
            set_attr::Args {
                file: handle,
                new_attr: sized_attr(None, Some(1)),
                guard: Some(set_attr::Guard { ctime: wrong_guard }),
            },
        )
        .await,
        "set_attr should fail with stale guard",
    );
    assert_eq!(fail.error, vfs::Error::NotSync);
    assert_eq!(stdfs::metadata(ctx.root_path().join("file.txt")).unwrap().len(), 2);
}

#[tokio::test]
async fn symlink_creates_symbolic_link() {
    let ctx = TestContext::new();
    let root = ctx.root_handle().await;

    let success = expect_ok(
        symlink::Symlink::symlink(
            &ctx.fs,
            symlink::Args {
                object: dir_op(root, "link.txt"),
                attr: sized_attr(Some(0o700), None),
                path: file_path("target.txt"),
            },
        )
        .await,
        "symlink should succeed",
    );
    assert!(success.file.is_some());
    assert!(matches!(success.attr.unwrap().file_type, file::Type::Symlink));
    assert_wcc_present(&success.wcc_data);
    assert_eq!(
        stdfs::read_link(ctx.root_path().join("link.txt")).unwrap(),
        PathBuf::from("target.txt")
    );
}

#[tokio::test]
async fn write_writes_data_with_offset_and_commit_matches_verifier() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "file.txt", b"");
    let root = ctx.root_handle().await;
    let handle = ctx.lookup_handle(root, "file.txt").await;

    let write_result = expect_ok(
        write::Write::write(
            &ctx.fs,
            write::Args {
                file: handle.clone(),
                offset: 2,
                size: 3,
                stable: write::StableHow::DataSync,
                data: slice_from_bytes(b"xyz").await,
            },
        )
        .await,
        "write should succeed",
    );
    assert_eq!(write_result.count, 3);
    assert_eq!(write_result.commited, write::StableHow::DataSync);
    assert_wcc_present(&write_result.file_wcc);
    assert_eq!(
        stdfs::read(ctx.root_path().join("file.txt")).unwrap(),
        vec![0, 0, b'x', b'y', b'z']
    );

    let commit_result = expect_ok(
        commit::Commit::commit(&ctx.fs, commit::Args { file: handle, offset: 0, count: 0 }).await,
        "commit after write should succeed",
    );
    assert_eq!(commit_result.verifier.0, write_result.verifier.0);
}

#[tokio::test]
async fn unstable_write_reports_unstable_and_commit_succeeds() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "file.txt", b"");
    let root = ctx.root_handle().await;
    let handle = ctx.lookup_handle(root, "file.txt").await;

    let write_result = expect_ok(
        write::Write::write(
            &ctx.fs,
            write::Args {
                file: handle.clone(),
                offset: 0,
                size: 4,
                stable: write::StableHow::Unstable,
                data: slice_from_bytes(b"data").await,
            },
        )
        .await,
        "unstable write should succeed",
    );
    assert_eq!(write_result.count, 4);
    assert_eq!(write_result.commited, write::StableHow::Unstable);

    let commit_result = expect_ok(
        commit::Commit::commit(&ctx.fs, commit::Args { file: handle, offset: 0, count: 0 }).await,
        "commit after unstable write should succeed",
    );
    assert_eq!(commit_result.verifier.0, write_result.verifier.0);
    assert_eq!(stdfs::read(ctx.root_path().join("file.txt")).unwrap(), b"data");
}

#[tokio::test]
async fn write_supports_segmented_slice_ranges() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "file.txt", b"");
    let root = ctx.root_handle().await;
    let handle = ctx.lookup_handle(root, "file.txt").await;

    let data = Slice::new(
        vec![
            b"abc".to_vec().into_boxed_slice(),
            b"def".to_vec().into_boxed_slice(),
            b"ghi".to_vec().into_boxed_slice(),
        ],
        1..8,
        None,
    );
    let write_result = expect_ok(
        write::Write::write(
            &ctx.fs,
            write::Args {
                file: handle,
                offset: 0,
                size: 5,
                stable: write::StableHow::Unstable,
                data,
            },
        )
        .await,
        "segmented write should succeed",
    );

    assert_eq!(write_result.count, 5);
    assert_eq!(stdfs::read(ctx.root_path().join("file.txt")).unwrap(), b"bcdef");
}

#[tokio::test]
async fn file_lifecycle_create_edit_read_and_remove() {
    let ctx = TestContext::new();
    let root = ctx.root_handle().await;

    let created = expect_ok(
        create::Create::create(
            &ctx.fs,
            create::Args {
                object: dir_op(root.clone(), "lifecycle.txt"),
                how: create::How::Guarded(default_new_attr()),
            },
        )
        .await,
        "file create should succeed",
    );
    let handle = created.file.expect("create must return a file handle");

    let write_result = expect_ok(
        write::Write::write(
            &ctx.fs,
            write::Args {
                file: handle.clone(),
                offset: 0,
                size: 11,
                stable: write::StableHow::FileSync,
                data: slice_from_bytes(b"hello world").await,
            },
        )
        .await,
        "file edit should succeed",
    );
    assert_eq!(write_result.count, 11);

    let read_result = expect_ok(
        read::Read::read(
            &ctx.fs,
            read::Args { file: handle.clone(), offset: 0, count: 11 },
            alloc_slice(11).await,
        )
        .await,
        "file read should succeed",
    );
    assert_eq!(read_result.head.count, 11);
    assert!(read_result.head.eof);
    assert_eq!(slice_to_vec(&read_result.data), b"hello world");

    let removed = expect_ok(
        remove::Remove::remove(&ctx.fs, remove::Args { object: dir_op(root, "lifecycle.txt") })
            .await,
        "file delete should succeed",
    );
    assert_wcc_present(&removed.wcc_data);
    assert!(!ctx.root_path().join("lifecycle.txt").exists());

    let stale = expect_err(
        get_attr::GetAttr::get_attr(&ctx.fs, get_attr::Args { file: handle }).await,
        "removed file handle should become stale",
    );
    assert_eq!(stale.error, vfs::Error::StaleFile);
}
