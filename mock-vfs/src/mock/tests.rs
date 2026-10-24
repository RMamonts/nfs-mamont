use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use nfs_mamont::auth::Credential;
use nfs_mamont::vfs::commit::Commit;
use nfs_mamont::vfs::create::Create;
use nfs_mamont::vfs::file::{Handle, Name, Type};
use nfs_mamont::vfs::fs_info::FsInfo;
use nfs_mamont::vfs::get_attr::GetAttr;
use nfs_mamont::vfs::lookup::Lookup;
use nfs_mamont::vfs::mk_dir::MkDir;
use nfs_mamont::vfs::read::Read;
use nfs_mamont::vfs::read_dir::{Cookie, CookieVerifier, ReadDir};
use nfs_mamont::vfs::read_dir_plus::ReadDirPlus;
use nfs_mamont::vfs::remove::Remove;
use nfs_mamont::vfs::rm_dir::RmDir;
use nfs_mamont::vfs::set_attr::{Guard, NewAttr, SetAttr, SetTime};
use nfs_mamont::vfs::write::{StableHow, Write};
use nfs_mamont::vfs::{self, commit, create, fs_info, get_attr, lookup, mk_dir, read, read_dir};
use nfs_mamont::vfs::{read_dir_plus, remove, rm_dir, set_attr, write, DirOpArgs};
use nfs_mamont::{Allocator, Buffer, Impl, Slice};

use super::MockVfs;
use crate::config::{Latency, MockVfsConfig};

const BACKEND: u8 = 3;
const CRED: Credential = Credential::None;

fn mock() -> MockVfs {
    MockVfs::new(MockVfsConfig {
        file_size: 10_000,
        files_per_dir: 3,
        dirs_per_dir: 2,
        ..MockVfsConfig::default()
    })
}

fn root() -> Handle {
    MockVfs::root_handle(BACKEND)
}

fn name(text: &str) -> Name {
    Name::new(text.to_string()).unwrap()
}

fn unset() -> NewAttr {
    NewAttr {
        mode: None,
        uid: None,
        gid: None,
        size: None,
        atime: SetTime::DontChange,
        mtime: SetTime::DontChange,
    }
}

async fn buffer(size: usize) -> Slice {
    let allocator = Impl::new(NonZeroUsize::new(4096).unwrap(), NonZeroUsize::new(8).unwrap());
    allocator.allocate(NonZeroUsize::new(size).unwrap()).await.unwrap()
}

async fn lookup(mock: &MockVfs, parent: &Handle, child: &str) -> lookup::Success {
    mock.lookup(lookup::Args { parent: parent.clone(), name: name(child) }, &CRED)
        .await
        .ok()
        .unwrap()
}

fn is_regular(attr: &vfs::file::Attr) -> bool {
    matches!(attr.file_type, Type::Regular)
}

fn is_directory(attr: &vfs::file::Attr) -> bool {
    matches!(attr.file_type, Type::Directory)
}

#[tokio::test]
async fn lookup_keeps_the_backend_and_decides_the_kind_by_name() {
    let mock = mock();

    let file = lookup(&mock, &root(), "file_0").await;
    assert_eq!(file.file.backend_id(), BACKEND);
    let attr = file.file_attr.unwrap();
    assert!(is_regular(&attr));
    assert_eq!(attr.size, 10_000);
    assert!(is_directory(&file.dir_attr.unwrap()));

    let dir = lookup(&mock, &root(), "dir_1").await;
    assert!(is_directory(&dir.file_attr.unwrap()));

    // Any name resolves, and resolving twice gives the same handle.
    let again = lookup(&mock, &root(), "file_0").await;
    assert_eq!(again.file, file.file);
    let other = lookup(&mock, &dir.file, "file_0").await;
    assert_ne!(other.file, file.file);
}

#[tokio::test]
async fn lookup_inside_a_file_is_rejected() {
    let mock = mock();
    let file = lookup(&mock, &root(), "file_0").await.file;

    let result = mock.lookup(lookup::Args { parent: file, name: name("x") }, &CRED).await;

    assert!(matches!(result, Err(lookup::Fail { error: vfs::Error::NotDir, .. })));
}

#[tokio::test]
async fn getattr_reports_the_object_kind() {
    let mock = mock();
    let dir = lookup(&mock, &root(), "dir_0").await.file;

    let root_attr = mock.get_attr(get_attr::Args { file: root() }, &CRED).await.ok().unwrap();
    let dir_attr = mock.get_attr(get_attr::Args { file: dir }, &CRED).await.ok().unwrap();

    assert!(is_directory(&root_attr.object));
    assert!(is_directory(&dir_attr.object));
    assert_ne!(root_attr.object.file_id, dir_attr.object.file_id);
}

#[tokio::test]
async fn read_is_bounded_by_the_buffer_and_the_file_size() {
    let mock = MockVfs::new(MockVfsConfig {
        file_size: 10_000,
        read_fill: Some(0xA5),
        ..MockVfsConfig::default()
    });
    let file = lookup(&mock, &root(), "file_0").await.file;

    let args = read::Args { file: file.clone(), offset: 0, count: 4096 };
    let success = mock.read(args, buffer(4096).await, &CRED).await.ok().unwrap();
    assert_eq!(success.head.count, 4096);
    assert!(!success.head.eof);
    assert!(success.data.chunks().flatten().all(|byte| *byte == 0xA5));

    let args = read::Args { file: file.clone(), offset: 9_000, count: 4096 };
    let success = mock.read(args, buffer(4096).await, &CRED).await.ok().unwrap();
    assert_eq!(success.head.count, 1_000);
    assert!(success.head.eof);

    let args = read::Args { file, offset: 20_000, count: 4096 };
    let success = mock.read(args, buffer(4096).await, &CRED).await.ok().unwrap();
    assert_eq!(success.head.count, 0);
    assert!(success.head.eof);
}

#[tokio::test]
async fn read_of_a_directory_is_rejected() {
    let mock = mock();

    let args = read::Args { file: root(), offset: 0, count: 16 };
    let result = mock.read(args, buffer(16).await, &CRED).await;

    assert!(matches!(result, Err(read::Fail { error: vfs::Error::InvalidArgument, .. })));
}

#[tokio::test]
async fn write_echoes_the_requested_stability() {
    let mock = mock();
    let file = lookup(&mock, &root(), "file_0").await.file;

    for stable in [StableHow::Unstable, StableHow::DataSync, StableHow::FileSync] {
        let args = write::Args {
            file: file.clone(),
            offset: 0,
            size: 4096,
            stable,
            data: buffer(4096).await,
        };
        let success = mock.write(args, &CRED).await.ok().unwrap();
        assert_eq!(success.count, 4096);
        assert_eq!(success.committed, stable);

        let committed = mock
            .commit(commit::Args { file: file.clone(), offset: 0, count: 0 }, &CRED)
            .await
            .ok()
            .unwrap();
        assert_eq!(committed.verifier.0, success.verifier.0);
    }
}

async fn list_all(mock: &MockVfs, count: u32) -> (Vec<String>, usize) {
    let mut names = Vec::new();
    let mut cookie = Cookie::new(0);
    let mut calls = 0;
    loop {
        let args = read_dir::Args {
            dir: root(),
            cookie,
            cookie_verifier: CookieVerifier::new([0; 8]),
            count,
        };
        let page = mock.read_dir(args, &CRED).await.ok().unwrap();
        calls += 1;
        for entry in page.entries {
            cookie = entry.cookie;
            names.push(entry.file_name.into_inner());
        }
        if page.eof {
            return (names, calls);
        }
    }
}

#[tokio::test]
async fn readdir_paginates_with_cookies() {
    let mock = mock();

    let (names, calls) = list_all(&mock, 4096).await;
    assert_eq!(names, [".", "..", "dir_0", "dir_1", "file_0", "file_1", "file_2"]);
    assert_eq!(calls, 1);

    let (names, calls) = list_all(&mock, 200).await;
    assert_eq!(names, [".", "..", "dir_0", "dir_1", "file_0", "file_1", "file_2"]);
    assert!(calls > 1, "a 200 byte budget must need several pages");
}

#[tokio::test]
async fn readdir_rejects_tiny_budgets_and_bad_cookies() {
    let mock = mock();

    let args = read_dir::Args {
        dir: root(),
        cookie: Cookie::new(0),
        cookie_verifier: CookieVerifier::new([0; 8]),
        count: 110,
    };
    let result = mock.read_dir(args, &CRED).await;
    assert!(matches!(result, Err(read_dir::Fail { error: vfs::Error::TooSmall, .. })));

    let args = read_dir::Args {
        dir: root(),
        cookie: Cookie::new(100),
        cookie_verifier: CookieVerifier::new([0; 8]),
        count: 4096,
    };
    let result = mock.read_dir(args, &CRED).await;
    assert!(matches!(result, Err(read_dir::Fail { error: vfs::Error::BadCookie, .. })));
}

#[tokio::test]
async fn readdirplus_returns_handles_matching_lookup() {
    let mock = mock();

    let args = read_dir_plus::Args {
        dir: root(),
        cookie: Cookie::new(0),
        cookie_verifier: CookieVerifier::new([0; 8]),
        dir_count: 4096,
        max_count: 16384,
    };
    let page = mock.read_dir_plus(args, &CRED).await.ok().unwrap();
    assert!(page.eof);
    assert_eq!(page.entries.len(), 7);

    for entry in page.entries.into_iter().skip(2) {
        let handle = entry.file_handle.unwrap();
        assert_eq!(handle.backend_id(), BACKEND);
        let looked_up = lookup(&mock, &root(), entry.file_name.as_str()).await;
        assert_eq!(looked_up.file, handle);
        let attr = entry.file_attr.unwrap();
        assert_eq!(attr.file_id, entry.file_id);
        assert_eq!(is_directory(&attr), entry.file_name.as_str().starts_with("dir"));
    }
}

#[tokio::test]
async fn create_and_mkdir_follow_the_naming_rule() {
    let mock = mock();
    let object = |name_text: &str| DirOpArgs { dir: root(), name: name(name_text) };

    let created = mock
        .create(
            create::Args { object: object("note.txt"), how: create::How::Unchecked(unset()) },
            &CRED,
        )
        .await
        .ok()
        .unwrap();
    let handle = created.file.unwrap();
    assert_eq!(handle, lookup(&mock, &root(), "note.txt").await.file);
    assert!(is_regular(&created.attr.unwrap()));

    let result = mock
        .create(
            create::Args { object: object("dir_new"), how: create::How::Unchecked(unset()) },
            &CRED,
        )
        .await;
    assert!(matches!(result, Err(create::Fail { error: vfs::Error::IsDir, .. })));

    let result = mock
        .create(
            create::Args { object: object("note.txt"), how: create::How::Guarded(unset()) },
            &CRED,
        )
        .await;
    assert!(matches!(result, Err(create::Fail { error: vfs::Error::Exist, .. })));

    let made = mock
        .mk_dir(mk_dir::Args { object: object("dir_new"), attr: unset() }, &CRED)
        .await
        .ok()
        .unwrap();
    assert!(is_directory(&made.attr.unwrap()));
    assert_eq!(made.file.unwrap(), lookup(&mock, &root(), "dir_new").await.file);

    let result =
        mock.mk_dir(mk_dir::Args { object: object("note.txt"), attr: unset() }, &CRED).await;
    assert!(matches!(result, Err(mk_dir::Fail { error: vfs::Error::Exist, .. })));
}

#[tokio::test]
async fn remove_and_rmdir_follow_the_naming_rule() {
    let mock = mock();
    let object = |name_text: &str| DirOpArgs { dir: root(), name: name(name_text) };

    assert!(mock.remove(remove::Args { object: object("file_1") }, &CRED).await.is_ok());
    let result = mock.remove(remove::Args { object: object("dir_1") }, &CRED).await;
    assert!(matches!(result, Err(remove::Fail { error: vfs::Error::IsDir, .. })));

    assert!(mock.rm_dir(rm_dir::Args { object: object("dir_1") }, &CRED).await.is_ok());
    let result = mock.rm_dir(rm_dir::Args { object: object("file_1") }, &CRED).await;
    assert!(matches!(result, Err(rm_dir::Fail { error: vfs::Error::NotDir, .. })));
}

#[tokio::test]
async fn set_attr_checks_the_ctime_guard() {
    let mock = mock();
    let attr = mock.get_attr(get_attr::Args { file: root() }, &CRED).await.ok().unwrap().object;

    let args = set_attr::Args {
        file: root(),
        new_attr: unset(),
        guard: Some(Guard { ctime: attr.ctime }),
    };
    assert!(mock.set_attr(args, &CRED).await.is_ok());

    let stale = vfs::file::Time { seconds: attr.ctime.seconds.wrapping_sub(1), nanos: 0 };
    let args =
        set_attr::Args { file: root(), new_attr: unset(), guard: Some(Guard { ctime: stale }) };
    let result = mock.set_attr(args, &CRED).await;
    assert!(matches!(result, Err(set_attr::Fail { error: vfs::Error::NotSync, .. })));
}

#[tokio::test]
async fn fs_info_advertises_the_configured_transfer_size() {
    let mock = MockVfs::new(MockVfsConfig { max_io: 65536, ..MockVfsConfig::default() });

    let info = mock.fs_info(fs_info::Args { root: root() }, &CRED).await.ok().unwrap();

    assert_eq!(info.read_max, 65536);
    assert_eq!(info.write_max, 65536);
    assert!(info.root_attr.is_some_and(|attr| is_directory(&attr)));
}

#[tokio::test]
async fn latency_is_paid_by_every_procedure() {
    let latency = Duration::from_millis(2);
    let mock = MockVfs::new(MockVfsConfig {
        latency: Latency::fixed(latency),
        ..MockVfsConfig::default()
    });

    let start = Instant::now();
    mock.get_attr(get_attr::Args { file: root() }, &CRED).await.ok().unwrap();
    assert!(start.elapsed() >= latency);

    let file = lookup(&mock, &root(), "file_0").await.file;
    let start = Instant::now();
    let args = read::Args { file, offset: 0, count: 16 };
    mock.read(args, buffer(16).await, &CRED).await.ok().unwrap();
    assert!(start.elapsed() >= latency);
}
