use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::{self, timeout};
use tokio_util::sync::CancellationToken;

use crate::backend::BackendRegistry;
use crate::consts::mount::{MOUNT_EXPORT, MOUNT_PROGRAM, MOUNT_VERSION};
use crate::consts::nfsv3::{NFS_PROGRAM, NFS_VERSION, NULL};
use crate::mount::Mount;
use crate::server::{serve, DEFAULT_SHUTDOWN_TIMEOUT};
use crate::service::nlm::NlmService;

use super::{allocator, assert_closed, call, reply_xid, GatedMount, StubVfs, WAIT};

/// [`serve`] running under test.
struct Supervised {
    addr: SocketAddr,
    shutdown: CancellationToken,
    handle: JoinHandle<std::io::Result<()>>,
}

impl Supervised {
    /// Starts [`serve`] on a free local port.
    async fn start<M>(mount_service: Arc<M>, shutdown_timeout: Duration) -> Self
    where
        M: Mount + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let shutdown = CancellationToken::new();

        let handle = tokio::spawn(serve(
            listener,
            allocator(),
            BackendRegistry::<StubVfs>::new(),
            mount_service,
            Arc::new(NlmService::new()),
            shutdown.clone(),
            shutdown_timeout,
        ));

        Self { addr, shutdown, handle }
    }

    /// Requests the shutdown and waits until [`serve`] returns.
    async fn stop(self) {
        self.shutdown.cancel();
        timeout(WAIT, self.handle).await.expect("server did not stop").unwrap().unwrap();
    }
}

#[tokio::test]
async fn shutdown_closes_connections_and_releases_port() {
    let server = Supervised::start(Arc::new(GatedMount::default()), DEFAULT_SHUTDOWN_TIMEOUT).await;
    let addr = server.addr;

    let mut idle = TcpStream::connect(addr).await.unwrap();
    let mut active = TcpStream::connect(addr).await.unwrap();
    active.write_all(&call(1, NFS_PROGRAM, NFS_VERSION, NULL)).await.unwrap();
    assert_eq!(timeout(WAIT, reply_xid(&mut active)).await.unwrap(), 1);

    server.stop().await;

    assert_closed(&mut idle).await;
    assert_closed(&mut active).await;
    TcpListener::bind(addr).await.expect("port is still taken");
}

#[tokio::test]
async fn shutdown_answers_in_flight_request() {
    let mount = Arc::new(GatedMount::default());
    let server = Supervised::start(Arc::clone(&mount), DEFAULT_SHUTDOWN_TIMEOUT).await;

    let mut client = TcpStream::connect(server.addr).await.unwrap();
    client.write_all(&call(7, MOUNT_PROGRAM, MOUNT_VERSION, MOUNT_EXPORT)).await.unwrap();
    timeout(WAIT, mount.entered.notified()).await.expect("EXPORT did not start");

    server.shutdown.cancel();
    // Give the server a chance to (wrongly) close the connection under the request.
    time::sleep(Duration::from_millis(50)).await;
    assert!(!server.handle.is_finished(), "server stopped before answering the request");

    mount.released.notify_one();
    assert_eq!(timeout(WAIT, reply_xid(&mut client)).await.expect("no reply"), 7);
    assert_closed(&mut client).await;
    server.stop().await;
}

#[tokio::test]
async fn shutdown_aborts_hung_request_after_timeout() {
    let mount = Arc::new(GatedMount::default());
    let server = Supervised::start(Arc::clone(&mount), Duration::from_millis(100)).await;

    let mut client = TcpStream::connect(server.addr).await.unwrap();
    client.write_all(&call(9, MOUNT_PROGRAM, MOUNT_VERSION, MOUNT_EXPORT)).await.unwrap();
    timeout(WAIT, mount.entered.notified()).await.expect("EXPORT did not start");

    // EXPORT is never released, so only the timeout can finish the shutdown.
    server.stop().await;

    assert_closed(&mut client).await;
}
