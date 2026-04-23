use nfs_mamont::vfs;
use nfs_mamont::vfs::file;
use nfs_mamont::vfs::get_attr;
use nfs_mamont::vfs::link;
use nfs_mamont::vfs::lookup;
use nfs_mamont::vfs::mk_dir;
use nfs_mamont::vfs::read_link;
use nfs_mamont::vfs::remove;
use nfs_mamont::vfs::rename;
use nfs_mamont::vfs::rm_dir;
use nfs_mamont::vfs::symlink;

use super::helpers::{
    assert_wcc_present, create_dir, dir_op, expect_err, expect_ok, file_path, name, write_file,
    MultiExportTestContext, TestContext,
};

#[tokio::test]
async fn lookup_resolves_child_and_rejects_non_directory_parent() {
    let ctx = TestContext::new();
    create_dir(ctx.root_path(), "dir");
    write_file(ctx.root_path(), "dir/child.txt", b"data");
    write_file(ctx.root_path(), "plain.txt", b"data");
    let root = ctx.root_handle().await;

    let dir = expect_ok(
        lookup::Lookup::lookup(&ctx.fs, lookup::Args { parent: root.clone(), name: name("dir") })
            .await,
        "lookup dir should succeed",
    );
    assert!(matches!(dir.file_attr.unwrap().file_type, file::Type::Directory));

    let child = expect_ok(
        lookup::Lookup::lookup(&ctx.fs, lookup::Args { parent: dir.file, name: name("child.txt") })
            .await,
        "lookup child should succeed",
    );
    assert!(matches!(child.file_attr.unwrap().file_type, file::Type::Regular));

    let plain = ctx.lookup_handle(root, "plain.txt").await;
    let fail = expect_err(
        lookup::Lookup::lookup(&ctx.fs, lookup::Args { parent: plain, name: name("nope") }).await,
        "lookup through non-directory should fail",
    );
    assert_eq!(fail.error, vfs::Error::NotDir);
}

#[tokio::test]
async fn lookup_resolves_dot_and_dotdot() {
    let ctx = TestContext::new();
    create_dir(ctx.root_path(), "dir/nested");
    let root = ctx.root_handle().await;
    let dir = ctx.lookup_handle(root.clone(), "dir").await;
    let nested = ctx.lookup_handle(dir.clone(), "nested").await;

    let dot = expect_ok(
        lookup::Lookup::lookup(&ctx.fs, lookup::Args { parent: dir.clone(), name: name(".") })
            .await,
        "lookup '.' should resolve to the same directory",
    );
    let dir_attr = expect_ok(
        get_attr::GetAttr::get_attr(&ctx.fs, get_attr::Args { file: dir.clone() }).await,
        "get_attr for directory should succeed",
    );
    let dot_attr = expect_ok(
        get_attr::GetAttr::get_attr(&ctx.fs, get_attr::Args { file: dot.file }).await,
        "get_attr for '.' result should succeed",
    );
    assert_eq!(dot_attr.object.file_id, dir_attr.object.file_id);

    let dotdot = expect_ok(
        lookup::Lookup::lookup(&ctx.fs, lookup::Args { parent: nested, name: name("..") }).await,
        "lookup '..' should resolve to the parent directory",
    );
    let dotdot_attr = expect_ok(
        get_attr::GetAttr::get_attr(&ctx.fs, get_attr::Args { file: dotdot.file }).await,
        "get_attr for '..' result should succeed",
    );
    assert_eq!(dotdot_attr.object.file_id, dir_attr.object.file_id);

    let root_parent = expect_ok(
        lookup::Lookup::lookup(&ctx.fs, lookup::Args { parent: root.clone(), name: name("..") })
            .await,
        "lookup '..' at export root should stay on export root",
    );
    let root_attr = expect_ok(
        get_attr::GetAttr::get_attr(&ctx.fs, get_attr::Args { file: root.clone() }).await,
        "get_attr for root should succeed",
    );
    let root_parent_attr = expect_ok(
        get_attr::GetAttr::get_attr(&ctx.fs, get_attr::Args { file: root_parent.file }).await,
        "get_attr for root '..' should succeed",
    );
    assert_eq!(root_parent_attr.object.file_id, root_attr.object.file_id);
}

#[tokio::test]
async fn remove_rejects_dot_and_dotdot() {
    let ctx = TestContext::new();
    let root = ctx.root_handle().await;

    let dot_fail = expect_err(
        remove::Remove::remove(&ctx.fs, remove::Args { object: dir_op(root.clone(), ".") }).await,
        "remove '.' should be rejected",
    );
    assert_eq!(dot_fail.error, vfs::Error::InvalidArgument);

    let dotdot_fail = expect_err(
        remove::Remove::remove(&ctx.fs, remove::Args { object: dir_op(root, "..") }).await,
        "remove '..' should be rejected",
    );
    assert_eq!(dotdot_fail.error, vfs::Error::Exist);
}

#[tokio::test]
async fn remove_deletes_file_and_invalidates_cached_handle() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "file.txt", b"hello");
    let root = ctx.root_handle().await;
    let file_handle = ctx.lookup_handle(root.clone(), "file.txt").await;

    let success = expect_ok(
        remove::Remove::remove(&ctx.fs, remove::Args { object: dir_op(root, "file.txt") }).await,
        "remove should succeed",
    );
    assert_wcc_present(&success.wcc_data);
    assert!(!ctx.root_path().join("file.txt").exists());

    let fail = expect_err(
        get_attr::GetAttr::get_attr(&ctx.fs, get_attr::Args { file: file_handle }).await,
        "removed handle should be stale",
    );
    assert_eq!(fail.error, vfs::Error::StaleFile);
}

#[tokio::test]
async fn rename_moves_subtree_and_updates_cached_descendants() {
    let ctx = TestContext::new();
    create_dir(ctx.root_path(), "dir/nested");
    write_file(ctx.root_path(), "dir/nested/file.txt", b"hello");
    let root = ctx.root_handle().await;
    let dir_handle = ctx.lookup_handle(root.clone(), "dir").await;
    let nested_handle = ctx.lookup_handle(dir_handle, "nested").await;
    let file_handle = ctx.lookup_handle(nested_handle, "file.txt").await;

    let success = expect_ok(
        rename::Rename::rename(
            &ctx.fs,
            rename::Args { from: dir_op(root.clone(), "dir"), to: dir_op(root.clone(), "moved") },
        )
        .await,
        "rename should succeed",
    );
    assert_wcc_present(&success.from_dir_wcc);
    assert_wcc_present(&success.to_dir_wcc);
    assert!(!ctx.root_path().join("dir").exists());
    assert!(ctx.root_path().join("moved/nested/file.txt").exists());

    let attr = expect_ok(
        get_attr::GetAttr::get_attr(&ctx.fs, get_attr::Args { file: file_handle }).await,
        "cached descendant handle should remain valid after rename",
    );
    assert!(matches!(attr.object.file_type, file::Type::Regular));

    let moved = expect_ok(
        lookup::Lookup::lookup(
            &ctx.fs,
            lookup::Args { parent: root.clone(), name: super::helpers::name("moved") },
        )
        .await,
        "lookup renamed directory should succeed",
    );
    assert!(matches!(moved.file_attr.unwrap().file_type, file::Type::Directory));

    let missing = expect_err(
        lookup::Lookup::lookup(&ctx.fs, lookup::Args { parent: root, name: name("dir") }).await,
        "old name should be gone after rename",
    );
    assert_eq!(missing.error, vfs::Error::NoEntry);
}

#[tokio::test]
async fn removing_one_hard_link_keeps_shared_handle_valid() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "original.txt", b"hello");
    std::fs::hard_link(ctx.root_path().join("original.txt"), ctx.root_path().join("alias.txt"))
        .unwrap();

    let root = ctx.root_handle().await;
    let original = ctx.lookup_handle(root.clone(), "original.txt").await;
    let alias = ctx.lookup_handle(root.clone(), "alias.txt").await;
    assert!(original == alias);

    let success = expect_ok(
        remove::Remove::remove(&ctx.fs, remove::Args { object: dir_op(root, "alias.txt") }).await,
        "remove hard-link alias should succeed",
    );
    assert_wcc_present(&success.wcc_data);

    let attr = expect_ok(
        get_attr::GetAttr::get_attr(&ctx.fs, get_attr::Args { file: original }).await,
        "shared handle should remain valid through surviving link",
    );
    assert!(matches!(attr.object.file_type, file::Type::Regular));
}

#[tokio::test]
async fn rename_replaces_existing_file_atomically() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "src.txt", b"new content");
    write_file(ctx.root_path(), "dst.txt", b"old content");
    let root = ctx.root_handle().await;

    let success = expect_ok(
        rename::Rename::rename(
            &ctx.fs,
            rename::Args {
                from: dir_op(root.clone(), "src.txt"),
                to: dir_op(root.clone(), "dst.txt"),
            },
        )
        .await,
        "rename onto existing file should succeed",
    );
    assert_wcc_present(&success.from_dir_wcc);
    assert!(!ctx.root_path().join("src.txt").exists());
    assert_eq!(std::fs::read(ctx.root_path().join("dst.txt")).unwrap(), b"new content");
}

#[tokio::test]
async fn rename_replaces_empty_directory() {
    let ctx = TestContext::new();
    create_dir(ctx.root_path(), "src_dir");
    write_file(ctx.root_path(), "src_dir/file.txt", b"data");
    create_dir(ctx.root_path(), "dst_dir");
    let root = ctx.root_handle().await;

    let success = expect_ok(
        rename::Rename::rename(
            &ctx.fs,
            rename::Args {
                from: dir_op(root.clone(), "src_dir"),
                to: dir_op(root.clone(), "dst_dir"),
            },
        )
        .await,
        "rename dir onto empty dir should succeed",
    );
    assert_wcc_present(&success.from_dir_wcc);
    assert!(ctx.root_path().join("dst_dir/file.txt").exists());
}

#[tokio::test]
async fn rename_rejects_type_mismatch() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "file.txt", b"data");
    create_dir(ctx.root_path(), "dir");
    let root = ctx.root_handle().await;

    let file_to_dir = expect_err(
        rename::Rename::rename(
            &ctx.fs,
            rename::Args {
                from: dir_op(root.clone(), "file.txt"),
                to: dir_op(root.clone(), "dir"),
            },
        )
        .await,
        "rename file onto directory should fail",
    );
    assert_eq!(file_to_dir.error, vfs::Error::Exist);

    let dir_to_file = expect_err(
        rename::Rename::rename(
            &ctx.fs,
            rename::Args { from: dir_op(root.clone(), "dir"), to: dir_op(root, "file.txt") },
        )
        .await,
        "rename directory onto file should fail",
    );
    assert_eq!(dir_to_file.error, vfs::Error::Exist);
}

#[tokio::test]
async fn rename_self_is_noop() {
    let ctx = TestContext::new();
    write_file(ctx.root_path(), "file.txt", b"data");
    let root = ctx.root_handle().await;

    let success = expect_ok(
        rename::Rename::rename(
            &ctx.fs,
            rename::Args { from: dir_op(root.clone(), "file.txt"), to: dir_op(root, "file.txt") },
        )
        .await,
        "rename file onto itself should be a no-op",
    );
    assert_wcc_present(&success.from_dir_wcc);
    assert_eq!(std::fs::read(ctx.root_path().join("file.txt")).unwrap(), b"data");
}

#[tokio::test]
async fn rm_dir_removes_empty_directory_and_rejects_non_empty_one() {
    let ctx = TestContext::new();
    create_dir(ctx.root_path(), "empty");
    create_dir(ctx.root_path(), "non-empty");
    write_file(ctx.root_path(), "non-empty/file.txt", b"data");
    let root = ctx.root_handle().await;

    let success = expect_ok(
        rm_dir::RmDir::rm_dir(&ctx.fs, rm_dir::Args { object: dir_op(root.clone(), "empty") })
            .await,
        "rm_dir should remove empty directories",
    );
    assert_wcc_present(&success.wcc_data);
    assert!(!ctx.root_path().join("empty").exists());

    let fail = expect_err(
        rm_dir::RmDir::rm_dir(&ctx.fs, rm_dir::Args { object: dir_op(root, "non-empty") }).await,
        "rm_dir should fail for non-empty directories",
    );
    assert_eq!(fail.error, vfs::Error::NotEmpty);
}

#[tokio::test]
async fn directory_lifecycle_create_symlink_rename_and_remove() {
    let ctx = TestContext::new();
    let root = ctx.root_handle().await;

    let created = expect_ok(
        mk_dir::MkDir::mk_dir(
            &ctx.fs,
            mk_dir::Args {
                object: dir_op(root.clone(), "docs"),
                attr: super::helpers::default_new_attr(),
            },
        )
        .await,
        "directory create should succeed",
    );
    let docs_handle = created.file.expect("mk_dir must return a handle");
    assert!(ctx.root_path().join("docs").is_dir());

    let link = expect_ok(
        symlink::Symlink::symlink(
            &ctx.fs,
            symlink::Args {
                object: dir_op(root.clone(), "docs-link"),
                attr: super::helpers::default_new_attr(),
                path: file_path("docs"),
            },
        )
        .await,
        "directory symlink should succeed",
    );
    let link_handle = link.file.expect("symlink must return a handle");

    let target = expect_ok(
        read_link::ReadLink::read_link(&ctx.fs, read_link::Args { file: link_handle }).await,
        "read_link for directory symlink should succeed",
    );
    assert_eq!(target.data.as_path(), std::path::Path::new("docs"));

    let renamed = expect_ok(
        rename::Rename::rename(
            &ctx.fs,
            rename::Args {
                from: dir_op(root.clone(), "docs"),
                to: dir_op(root.clone(), "docs-renamed"),
            },
        )
        .await,
        "directory rename should succeed",
    );
    assert_wcc_present(&renamed.from_dir_wcc);
    assert!(ctx.root_path().join("docs-renamed").is_dir());

    write_file(ctx.root_path(), "docs-renamed/note.txt", b"data");
    let attr = expect_ok(
        get_attr::GetAttr::get_attr(&ctx.fs, get_attr::Args { file: docs_handle.clone() }).await,
        "renamed directory handle should remain valid",
    );
    assert!(matches!(attr.object.file_type, file::Type::Directory));

    let removed_link = expect_ok(
        remove::Remove::remove(&ctx.fs, remove::Args { object: dir_op(root.clone(), "docs-link") })
            .await,
        "symlink removal should succeed",
    );
    assert_wcc_present(&removed_link.wcc_data);

    let removed_file = expect_ok(
        remove::Remove::remove(
            &ctx.fs,
            remove::Args { object: dir_op(docs_handle.clone(), "note.txt") },
        )
        .await,
        "file inside renamed directory should be removable",
    );
    assert_wcc_present(&removed_file.wcc_data);

    let removed_dir = expect_ok(
        rm_dir::RmDir::rm_dir(&ctx.fs, rm_dir::Args { object: dir_op(root, "docs-renamed") }).await,
        "empty renamed directory should be removable",
    );
    assert_wcc_present(&removed_dir.wcc_data);
    assert!(!ctx.root_path().join("docs-renamed").exists());
}

#[tokio::test]
async fn lookup_dotdot_stays_inside_each_export_root() {
    let ctx = MultiExportTestContext::new(2);
    create_dir(ctx.root_path(0), "nested");
    create_dir(ctx.root_path(1), "nested");

    let export_a_root = ctx.root_handle(0).await;
    let export_b_root = ctx.root_handle(1).await;
    let export_a_nested = ctx.lookup_handle(export_a_root.clone(), "nested").await;
    let export_b_nested = ctx.lookup_handle(export_b_root.clone(), "nested").await;

    let export_a_parent = expect_ok(
        lookup::Lookup::lookup(&ctx.fs, lookup::Args { parent: export_a_nested, name: name("..") })
            .await,
        "lookup '..' in first export should resolve to first root",
    );
    let export_b_parent = expect_ok(
        lookup::Lookup::lookup(&ctx.fs, lookup::Args { parent: export_b_nested, name: name("..") })
            .await,
        "lookup '..' in second export should resolve to second root",
    );

    assert_eq!(export_a_parent.file, export_a_root);
    assert_eq!(export_b_parent.file, export_b_root);
}

#[tokio::test]
async fn link_rejects_cross_export_targets() {
    let ctx = MultiExportTestContext::new(2);
    write_file(ctx.root_path(0), "source.txt", b"hello");

    let source_root = ctx.root_handle(0).await;
    let target_root = ctx.root_handle(1).await;
    let source = ctx.lookup_handle(source_root, "source.txt").await;

    let fail = expect_err(
        link::Link::link(
            &ctx.fs,
            link::Args { file: source, link: dir_op(target_root, "linked.txt") },
        )
        .await,
        "cross-export hard link should fail",
    );
    assert_eq!(fail.error, vfs::Error::XDev);
}

#[tokio::test]
async fn rename_rejects_cross_export_targets() {
    let ctx = MultiExportTestContext::new(2);
    write_file(ctx.root_path(0), "source.txt", b"hello");

    let source_root = ctx.root_handle(0).await;
    let target_root = ctx.root_handle(1).await;

    let fail = expect_err(
        rename::Rename::rename(
            &ctx.fs,
            rename::Args {
                from: dir_op(source_root, "source.txt"),
                to: dir_op(target_root, "moved.txt"),
            },
        )
        .await,
        "cross-export rename should fail",
    );
    assert_eq!(fail.error, vfs::Error::XDev);
}
