use std::io;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Barrier;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use nfs_mamont::mount::{MOUNT_PROGRAM, MOUNT_VERSION};
use nfs_mamont::nfsv3::{NFS_PROGRAM, NFS_VERSION};
use nfs_mamont::rpc::{
    RpcBody, ServerContext, ServerExport, ServerSettings, SharedVfs, RPC_VERSION,
};
use nfs_mamont::vfs;
use nfs_mamont::vfs::file;

const RECORD_LAST_FRAGMENT_BIT: u32 = 0x8000_0000;
const AUTH_NONE: u32 = 0;
const NULL_PROCEDURE: u32 = 0;
const MOUNT_PROCEDURE: u32 = 1;

fn non_zero(value: usize) -> NonZeroUsize {
    match NonZeroUsize::new(value) {
        Some(value) => value,
        None => panic!("test helper expects non-zero value"),
    }
}

fn push_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend(value.to_be_bytes());
}

fn push_opaque(buf: &mut Vec<u8>, bytes: &[u8]) {
    push_u32(buf, bytes.len() as u32);
    buf.extend(bytes);
    let padding = (4 - bytes.len() % 4) % 4;
    buf.extend(std::iter::repeat_n(0_u8, padding));
}

fn null_call_frame(xid: u32) -> Vec<u8> {
    let mut body = Vec::new();
    push_u32(&mut body, xid);
    push_u32(&mut body, RpcBody::Call as u32);
    push_u32(&mut body, RPC_VERSION);
    push_u32(&mut body, NFS_PROGRAM);
    push_u32(&mut body, NFS_VERSION);
    push_u32(&mut body, NULL_PROCEDURE);
    push_u32(&mut body, AUTH_NONE);
    push_u32(&mut body, 0);
    push_u32(&mut body, AUTH_NONE);
    push_u32(&mut body, 0);

    let mut frame = Vec::with_capacity(body.len() + size_of::<u32>());
    push_u32(&mut frame, RECORD_LAST_FRAGMENT_BIT | body.len() as u32);
    frame.extend(body);
    frame
}

fn mount_call_frame(xid: u32, export: &str) -> Vec<u8> {
    let mut body = Vec::new();
    push_u32(&mut body, xid);
    push_u32(&mut body, RpcBody::Call as u32);
    push_u32(&mut body, RPC_VERSION);
    push_u32(&mut body, MOUNT_PROGRAM);
    push_u32(&mut body, MOUNT_VERSION);
    push_u32(&mut body, MOUNT_PROCEDURE);
    push_u32(&mut body, AUTH_NONE);
    push_u32(&mut body, 0);
    push_u32(&mut body, AUTH_NONE);
    push_u32(&mut body, 0);
    push_opaque(&mut body, export.as_bytes());

    let mut frame = Vec::with_capacity(body.len() + size_of::<u32>());
    push_u32(&mut frame, RECORD_LAST_FRAGMENT_BIT | body.len() as u32);
    frame.extend(body);
    frame
}

async fn read_record(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut header = [0_u8; 4];
    stream.read_exact(&mut header).await?;

    let length = u32::from_be_bytes(header) & !RECORD_LAST_FRAGMENT_BIT;
    let mut payload = vec![0_u8; length as usize];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}

fn reply_xid(reply: &[u8]) -> u32 {
    u32::from_be_bytes(reply[..4].try_into().expect("reply must contain xid"))
}

async fn start_server(
    settings: ServerSettings,
) -> io::Result<(std::net::SocketAddr, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let _ = nfs_mamont::handle_forever_with_context(
            listener,
            ServerContext::with_settings(settings),
        )
        .await;
    });
    Ok((local_addr, server))
}

async fn start_server_with_context(
    server_context: ServerContext,
) -> io::Result<(std::net::SocketAddr, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let _ = nfs_mamont::handle_forever_with_context(listener, server_context).await;
    });
    Ok((local_addr, server))
}

struct SlowRootBackend {
    entered_calls: AtomicUsize,
    started_barrier: Arc<Barrier>,
}

impl SlowRootBackend {
    fn new(started_barrier: Arc<Barrier>) -> Self {
        Self { entered_calls: AtomicUsize::new(0), started_barrier }
    }

    fn entered_calls(&self) -> usize {
        self.entered_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl vfs::RootHandle for SlowRootBackend {
    async fn root_handle(&self) -> file::Handle {
        let started_calls = self.entered_calls.fetch_add(1, Ordering::SeqCst);
        if started_calls == 0 {
            self.started_barrier.wait().await;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        file::Handle([1_u8; nfs_mamont::nfsv3::NFS3_FHSIZE])
    }
}

macro_rules! panic_vfs_impl {
    ($trait_path:path, $method:ident, $args_ty:path, $result_ty:path) => {
        #[async_trait]
        impl $trait_path for SlowRootBackend {
            async fn $method(&self, _args: $args_ty) -> $result_ty {
                panic!(concat!(stringify!($method), " should not be called in this test"));
            }
        }
    };
}

panic_vfs_impl!(vfs::access::Access, access, vfs::access::Args, vfs::access::Result);
panic_vfs_impl!(vfs::commit::Commit, commit, vfs::commit::Args, vfs::commit::Result);
panic_vfs_impl!(vfs::create::Create, create, vfs::create::Args, vfs::create::Result);
panic_vfs_impl!(vfs::fs_info::FsInfo, fs_info, vfs::fs_info::Args, vfs::fs_info::Result);
panic_vfs_impl!(vfs::fs_stat::FsStat, fs_stat, vfs::fs_stat::Args, vfs::fs_stat::Result);
panic_vfs_impl!(vfs::get_attr::GetAttr, get_attr, vfs::get_attr::Args, vfs::get_attr::Result);
panic_vfs_impl!(vfs::link::Link, link, vfs::link::Args, vfs::link::Result);
panic_vfs_impl!(vfs::lookup::Lookup, lookup, vfs::lookup::Args, vfs::lookup::Result);
panic_vfs_impl!(vfs::mk_dir::MkDir, mk_dir, vfs::mk_dir::Args, vfs::mk_dir::Result);
panic_vfs_impl!(vfs::mk_node::MkNode, mk_node, vfs::mk_node::Args, vfs::mk_node::Result);
panic_vfs_impl!(vfs::path_conf::PathConf, path_conf, vfs::path_conf::Args, vfs::path_conf::Result);
panic_vfs_impl!(vfs::read::Read, read, vfs::read::Args, vfs::read::Result);
panic_vfs_impl!(vfs::read_dir::ReadDir, read_dir, vfs::read_dir::Args, vfs::read_dir::Result);
panic_vfs_impl!(
    vfs::read_dir_plus::ReadDirPlus,
    read_dir_plus,
    vfs::read_dir_plus::Args,
    vfs::read_dir_plus::Result
);
panic_vfs_impl!(vfs::read_link::ReadLink, read_link, vfs::read_link::Args, vfs::read_link::Result);
panic_vfs_impl!(vfs::remove::Remove, remove, vfs::remove::Args, vfs::remove::Result);
panic_vfs_impl!(vfs::rename::Rename, rename, vfs::rename::Args, vfs::rename::Result);
panic_vfs_impl!(vfs::rm_dir::RmDir, rm_dir, vfs::rm_dir::Args, vfs::rm_dir::Result);
panic_vfs_impl!(vfs::set_attr::SetAttr, set_attr, vfs::set_attr::Args, vfs::set_attr::Result);
panic_vfs_impl!(vfs::symlink::Symlink, symlink, vfs::symlink::Args, vfs::symlink::Result);
panic_vfs_impl!(vfs::write::Write, write, vfs::write::Args, vfs::write::Result);

#[tokio::test]
async fn server_accepts_new_connection_after_client_disconnect() {
    let (addr, server) = start_server(ServerSettings::new()).await.expect("server should start");

    let mut first_client = TcpStream::connect(addr).await.expect("first client should connect");
    first_client.write_all(&null_call_frame(1)).await.expect("first request should be written");

    let first_reply = timeout(Duration::from_secs(1), read_record(&mut first_client))
        .await
        .expect("first reply should arrive")
        .expect("first reply should be readable");
    assert_eq!(reply_xid(&first_reply), 1);

    drop(first_client);

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut second_client = TcpStream::connect(addr).await.expect("second client should connect");
    second_client.write_all(&null_call_frame(2)).await.expect("second request should be written");

    let second_reply = timeout(Duration::from_secs(1), read_record(&mut second_client))
        .await
        .expect("second reply should arrive")
        .expect("second reply should be readable");
    assert_eq!(reply_xid(&second_reply), 2);

    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn bounded_connection_queues_preserve_all_null_replies() {
    let settings = ServerSettings::new()
        .with_command_queue_size(non_zero(1))
        .with_result_queue_size(non_zero(1))
        .with_max_in_flight_requests(non_zero(2));
    let (addr, server) = start_server(settings).await.expect("server should start");

    let mut client = TcpStream::connect(addr).await.expect("client should connect");
    let request_stream = (1..=8_u32).flat_map(null_call_frame).collect::<Vec<u8>>();

    client.write_all(&request_stream).await.expect("requests should be written");

    for expected_xid in 1..=8_u32 {
        let reply = timeout(Duration::from_secs(1), read_record(&mut client))
            .await
            .expect("reply should arrive")
            .expect("reply should be readable");
        assert_eq!(reply_xid(&reply), expected_xid);
    }

    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn slow_backend_mount_requests_are_backpressured_per_connection() {
    let settings = ServerSettings::new()
        .with_command_queue_size(non_zero(1))
        .with_result_queue_size(non_zero(1))
        .with_max_in_flight_requests(non_zero(1));
    let started_barrier = Arc::new(Barrier::new(2));
    let backend = Arc::new(SlowRootBackend::new(started_barrier.clone()));
    let shared_backend: SharedVfs = backend.clone();
    let context = ServerContext::with_backend_and_settings(shared_backend, settings);
    let export_path =
        file::Path::new("/tmp/export".to_string()).expect("export path should be valid");
    context.add_export(ServerExport::new(export_path.clone(), vec!["*".to_string()])).await;

    let (addr, server) = start_server_with_context(context).await.expect("server should start");
    let mut client = TcpStream::connect(addr).await.expect("client should connect");
    let request_stream = (1..=3_u32)
        .flat_map(|xid| {
            mount_call_frame(xid, export_path.as_path().to_str().expect("utf8 export path"))
        })
        .collect::<Vec<u8>>();

    client.write_all(&request_stream).await.expect("requests should be written");

    timeout(Duration::from_secs(1), started_barrier.wait())
        .await
        .expect("first mount should enter backend");
    assert_eq!(backend.entered_calls(), 1);

    let started = tokio::time::Instant::now();
    let first_reply = timeout(Duration::from_secs(2), read_record(&mut client))
        .await
        .expect("first mount reply should arrive")
        .expect("first mount reply should be readable");
    assert_eq!(reply_xid(&first_reply), 1);
    assert!(started.elapsed() >= Duration::from_millis(150));

    let second_reply = timeout(Duration::from_secs(2), read_record(&mut client))
        .await
        .expect("second mount reply should arrive")
        .expect("second mount reply should be readable");
    assert_eq!(reply_xid(&second_reply), 2);
    assert!(started.elapsed() >= Duration::from_millis(350));

    let third_reply = timeout(Duration::from_secs(2), read_record(&mut client))
        .await
        .expect("third mount reply should arrive")
        .expect("third mount reply should be readable");
    assert_eq!(reply_xid(&third_reply), 3);
    assert!(started.elapsed() >= Duration::from_millis(550));
    assert_eq!(backend.entered_calls(), 3);

    server.abort();
    let _ = server.await;
}
