//! End-to-end tests: a real server in-process, the real parser and serializer on the
//! wire, and the client of this crate talking to them.

use std::time::{Duration, Instant};

use mock_vfs::client::{self, mount, nfs, Connection};
use mock_vfs::config::{Latency, MockVfsConfig};
use mock_vfs::server::{Server, ServerConfig};

const FILE_SIZE: u64 = 1 << 20;

async fn start(latency: Duration) -> Server {
    let config = ServerConfig {
        vfs: MockVfsConfig {
            latency: Latency::fixed(latency),
            file_size: FILE_SIZE,
            files_per_dir: 3,
            dirs_per_dir: 2,
            ..MockVfsConfig::default()
        },
        ..ServerConfig::default()
    };

    Server::start(config).await.expect("the server starts on a loopback port")
}

async fn connect_and_mount(server: &Server) -> (Connection, Vec<u8>) {
    let mut conn = Connection::connect(server.addr()).await.expect("connect");
    let root = mount::mount(&mut conn, "/").await.expect("MNT /");

    (conn, root)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mount_lookup_read_write_over_the_wire() {
    let server = start(Duration::ZERO).await;
    let (mut conn, root) = connect_and_mount(&server).await;
    assert_eq!(root[0], server.backend_id(), "root handle carries the backend id");

    let file = nfs::lookup(&mut conn, &root, "file_0").await.unwrap();
    let attr = file.attr.expect("LOOKUP returns attributes");
    assert_eq!(attr.file_type, nfs::TYPE_REGULAR);
    assert_eq!(attr.size, FILE_SIZE);
    assert_eq!(nfs::getattr(&mut conn, &file.handle).await.unwrap(), attr);

    let read = nfs::read(&mut conn, &file.handle, 0, 8192).await.unwrap();
    assert_eq!(read.count, 8192);
    assert_eq!(read.data.len(), 8192);
    assert!(!read.eof);

    let tail = nfs::read(&mut conn, &file.handle, FILE_SIZE - 100, 8192).await.unwrap();
    assert_eq!(tail.count, 100);
    assert!(tail.eof);

    let data = vec![7u8; 4096];
    let write = nfs::write(&mut conn, &file.handle, 4096, &data, nfs::UNSTABLE).await.unwrap();
    assert_eq!(write.count, 4096);
    assert_eq!(write.committed, nfs::UNSTABLE);
    let verifier = nfs::commit(&mut conn, &file.handle, 0, 0).await.unwrap();
    assert_eq!(verifier, write.verifier);

    let info = nfs::fsinfo(&mut conn, &root).await.unwrap();
    assert_eq!(info.rtmax, 1 << 20);
    assert_eq!(info.wtmax, 1 << 20);

    let rights = nfs::access(&mut conn, &file.handle, nfs::ACCESS_ALL).await.unwrap();
    assert_eq!(rights, nfs::ACCESS_ALL);

    let created = nfs::create(&mut conn, &root, "new.bin").await.unwrap().expect("handle");
    let looked_up = nfs::lookup(&mut conn, &root, "new.bin").await.unwrap();
    assert_eq!(created, looked_up.handle);
    nfs::remove(&mut conn, &root, "new.bin").await.unwrap();

    let err = nfs::lookup(&mut conn, &file.handle, "x").await.unwrap_err();
    assert!(matches!(err, client::Error::Status(20)), "NOTDIR expected, got {err}");

    nfs::null(&mut conn).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn readdir_pages_through_the_whole_directory() {
    let server = start(Duration::ZERO).await;
    let (mut conn, root) = connect_and_mount(&server).await;

    let mut names = Vec::new();
    let mut cookie = 0;
    let mut verifier = [0u8; 8];
    let mut pages = 0;
    loop {
        let page = nfs::readdir(&mut conn, &root, cookie, &verifier, 160).await.unwrap();
        pages += 1;
        for entry in page.entries {
            cookie = entry.cookie;
            names.push(entry.name);
        }
        verifier = page.verifier;
        if page.eof {
            break;
        }
    }
    assert_eq!(names, [".", "..", "dir_0", "dir_1", "file_0", "file_1", "file_2"]);
    assert!(pages > 1, "a 160 byte budget needs several pages");

    let plus = nfs::readdirplus(&mut conn, &root, 0, &[0u8; 8], 4096, 32768).await.unwrap();
    assert!(plus.eof);
    assert_eq!(plus.entries.len(), 7);
    for entry in &plus.entries[2..] {
        let handle = entry.handle.as_ref().expect("READDIRPLUS returns handles");
        let attr = entry.attr.expect("READDIRPLUS returns attributes");
        assert_eq!(attr.file_type == nfs::TYPE_DIRECTORY, entry.name.starts_with("dir"));
        assert_eq!(attr.fileid, entry.fileid);
        let looked_up = nfs::lookup(&mut conn, &root, &entry.name).await.unwrap();
        assert_eq!(&looked_up.handle, handle);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn backend_latency_is_visible_to_the_client() {
    let latency = Duration::from_millis(5);
    let server = start(latency).await;
    let (mut conn, root) = connect_and_mount(&server).await;

    let start = Instant::now();
    let file = nfs::lookup(&mut conn, &root, "file_0").await.unwrap();
    assert!(start.elapsed() >= latency, "LOOKUP answered before the backend latency");

    let start = Instant::now();
    nfs::read(&mut conn, &file.handle, 0, 4096).await.unwrap();
    assert!(start.elapsed() >= latency, "READ answered before the backend latency");

    // Requests on different connections are served by different VFS workers, so they
    // overlap instead of queueing behind each other.
    let connections = 8;
    let start = Instant::now();
    let mut tasks = Vec::new();
    for _ in 0..connections {
        let addr = server.addr();
        let handle = file.handle.clone();
        tasks.push(tokio::spawn(async move {
            let mut conn = Connection::connect(addr).await.unwrap();
            nfs::getattr(&mut conn, &handle).await.unwrap();
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    let elapsed = start.elapsed();
    assert!(elapsed >= latency);
    assert!(
        elapsed < latency * (connections - 2),
        "{connections} parallel requests took {elapsed:?}, as if they were serialized"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mounting_an_unknown_export_is_denied() {
    let server = start(Duration::ZERO).await;
    let mut conn = Connection::connect(server.addr()).await.unwrap();

    let err = mount::mount(&mut conn, "/nope").await.unwrap_err();

    assert!(matches!(err, client::Error::Status(13)), "MNT3ERR_ACCES expected, got {err}");
}
