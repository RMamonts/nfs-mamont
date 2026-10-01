use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use crate::allocator::Impl;
use crate::backend::BackendRegistry;
use crate::consts::nfsv3::{NFS_PROGRAM, NFS_VERSION, NULL};
use crate::server::Server;
use crate::service::mount::MountService;

use super::{allocator, assert_closed, call, reply_xid, StubVfs, WAIT};

/// Returns a stopped server bound to `addr` that leaves the system rpcbind alone.
fn server(addr: SocketAddr) -> Server<Impl, StubVfs> {
    Server::builder(allocator()).bind(addr).rpcbind(false).build()
}

/// Returns a loopback address on which the OS picks a free port.
fn any_port() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

/// Connects to `addr` and checks that an NFS `NULL` call is answered.
async fn ping(addr: SocketAddr) -> TcpStream {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(&call(1, NFS_PROGRAM, NFS_VERSION, NULL)).await.unwrap();
    assert_eq!(timeout(WAIT, reply_xid(&mut stream)).await.expect("no reply"), 1);

    stream
}

/// Stops `server`, failing the test if the shutdown hangs.
async fn stop(server: &mut Server<Impl, StubVfs>) {
    timeout(WAIT, server.stop()).await.expect("server did not stop").unwrap();
}

#[test]
fn builder_shares_given_registry_and_services() {
    let backends = BackendRegistry::new();
    let mount = Arc::new(MountService::with_exports(Vec::new()));
    let server =
        Server::builder(allocator()).backends(backends.clone()).mount(Arc::clone(&mount)).build();

    let id = backends.add(Arc::new(StubVfs)).unwrap();

    assert!(server.backends().get(id).is_some());
    assert!(Arc::ptr_eq(server.mount_service(), &mount));
}

#[tokio::test]
async fn serves_from_start_until_stop() {
    let mut server = server(any_port());
    assert!(!server.is_running());
    assert_eq!(server.local_addr(), None);

    let addr = server.start().await.unwrap();
    assert!(server.is_running());
    assert_eq!(server.local_addr(), Some(addr));
    let mut client = ping(addr).await;

    stop(&mut server).await;
    assert!(!server.is_running());
    assert_eq!(server.local_addr(), None);
    assert_closed(&mut client).await;
    TcpListener::bind(addr).await.expect("port is still taken");
}

#[tokio::test]
async fn restarts_on_the_same_port() {
    // Reserve a free port, so that every start binds the same address.
    let addr = TcpListener::bind(any_port()).await.unwrap().local_addr().unwrap();
    let mut server = server(addr);

    for _ in 0..2 {
        assert_eq!(server.start().await.unwrap(), addr);
        // The server closes the connection first, leaving its side in TIME_WAIT.
        let mut client = ping(addr).await;
        stop(&mut server).await;
        assert_closed(&mut client).await;
    }
}

#[tokio::test]
async fn start_fails_when_already_started() {
    let mut server = server(any_port());
    let addr = server.start().await.unwrap();

    let err = server.start().await.unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(server.local_addr(), Some(addr));
    ping(addr).await;

    stop(&mut server).await;
}

#[tokio::test]
async fn start_reports_bind_error() {
    let taken = TcpListener::bind(any_port()).await.unwrap();
    let mut server = server(taken.local_addr().unwrap());

    let err = server.start().await.unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AddrInUse);
    assert!(!server.is_running());
}

#[tokio::test]
async fn stop_is_idempotent() {
    let mut server = server(any_port());
    stop(&mut server).await;

    server.start().await.unwrap();
    stop(&mut server).await;
    stop(&mut server).await;
}

#[tokio::test]
async fn drop_aborts_running_server() {
    let mut server = server(any_port());
    let addr = server.start().await.unwrap();
    let mut client = ping(addr).await;

    drop(server);

    assert_closed(&mut client).await;
}
