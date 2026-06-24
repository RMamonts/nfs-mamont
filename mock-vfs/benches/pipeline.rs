use std::num::NonZeroUsize;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

use nfs_mamont::{handle_forever, Impl, ServerContext};

use mock_vfs::config::MockVfsConfig;
use mock_vfs::mock::{MockMount, MockVfs};
use mock_vfs::xdr::{build_rpc_frame, pad, push_opaque, push_u32, push_u64};

const XID: u32 = 42;

// ── Request builders ───────────────────────────────────────────────────

const HANDLE: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

fn getattr_req() -> Vec<u8> {
    let mut a = Vec::new();
    push_opaque(&mut a, &HANDLE);
    build_rpc_frame(1, XID, &a)
}

fn read_req(count: u32) -> Vec<u8> {
    let mut a = Vec::new();
    push_opaque(&mut a, &HANDLE);
    push_u64(&mut a, 0);
    push_u32(&mut a, count);
    build_rpc_frame(6, XID, &a)
}

fn write_req(data: &[u8]) -> Vec<u8> {
    let mut a = Vec::new();
    push_opaque(&mut a, &HANDLE);
    push_u64(&mut a, 0);
    push_u32(&mut a, data.len() as u32);
    push_u32(&mut a, 2);
    push_opaque(&mut a, data);
    build_rpc_frame(7, XID, &a)
}

fn lookup_req() -> Vec<u8> {
    let mut a = Vec::new();
    push_opaque(&mut a, &HANDLE);
    push_u32(&mut a, 4);
    a.extend_from_slice(b"file");
    pad(&mut a, 4);
    build_rpc_frame(3, XID, &a)
}

fn readdir_req() -> Vec<u8> {
    let mut a = Vec::new();
    push_opaque(&mut a, &HANDLE);
    push_u64(&mut a, 0);
    push_u64(&mut a, 0);
    push_u32(&mut a, 8192);
    build_rpc_frame(16, XID, &a)
}

fn commit_req() -> Vec<u8> {
    let mut a = Vec::new();
    push_opaque(&mut a, &HANDLE);
    push_u64(&mut a, 0);
    push_u32(&mut a, 0);
    build_rpc_frame(21, XID, &a)
}

// ── Response reader ────────────────────────────────────────────────────

async fn read_response(stream: &mut TcpStream) -> Vec<u8> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await.unwrap();
    let frag = u32::from_be_bytes(header);
    let len = (frag & 0x7FFF_FFFF) as usize;
    let mut resp = vec![0u8; len];
    stream.read_exact(&mut resp).await.unwrap();
    resp
}

// ── Benchmark helper ───────────────────────────────────────────────────

fn bench_procedure(
    c: &mut Criterion,
    name: &str,
    rt: &tokio::runtime::Runtime,
    addr: &std::net::SocketAddr,
    req: Vec<u8>,
) {
    let stream = rt.block_on(TcpStream::connect(addr)).unwrap();
    let shared = Arc::new(Mutex::new(stream));

    c.bench_function(name, |b| {
        b.to_async(rt).iter(|| {
            let stream = Arc::clone(&shared);
            let req = req.clone();
            async move {
                let mut guard = stream.lock().await;
                guard.write_all(&req).await.unwrap();
                let _resp = read_response(&mut guard).await;
            }
        });
    });
}

// ── Benchmark group ────────────────────────────────────────────────────

fn bench_nfs_pipeline(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    let addr = if let Ok(target) = std::env::var("MOCK_TARGET") {
        eprintln!("Remote mode: connecting to {target}");
        target
            .parse::<std::net::SocketAddr>()
            .expect("MOCK_TARGET must be a valid SocketAddr (e.g. 192.168.1.100:2049)")
    } else {
        eprintln!("Local mode: starting embedded server");
        let config = MockVfsConfig::default();
        let backend = Arc::new(MockVfs::new(config));
        let buf_size = NonZeroUsize::new(1048576).unwrap();
        let buf_count = NonZeroUsize::new(64).unwrap();
        let read_alloc = Arc::new(Impl::new(buf_size, buf_count));
        let write_alloc = Arc::new(Impl::new(buf_size, buf_count));
        let pool_size = NonZeroUsize::new(4).unwrap();
        let context = ServerContext::new(backend, read_alloc, write_alloc, pool_size);
        let mount_service = Arc::new(MockMount);

        let listener = rt.block_on(TcpListener::bind("127.0.0.1:0")).unwrap();
        let addr = rt.block_on(async { listener.local_addr() }).unwrap();

        rt.spawn(async move {
            handle_forever(listener, context, mount_service).await.unwrap();
        });

        for _ in 0..100 {
            if rt.block_on(TcpStream::connect(addr)).is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        addr
    };

    bench_procedure(c, "getattr", &rt, &addr, getattr_req());
    bench_procedure(c, "read_4k", &rt, &addr, read_req(4096));
    bench_procedure(c, "read_64k", &rt, &addr, read_req(65536));
    bench_procedure(c, "read_1m", &rt, &addr, read_req(1048576));
    {
        let data = vec![0xABu8; 65536];
        bench_procedure(c, "write_64k", &rt, &addr, write_req(&data));
    }
    bench_procedure(c, "lookup", &rt, &addr, lookup_req());
    bench_procedure(c, "readdir", &rt, &addr, readdir_req());
    bench_procedure(c, "commit", &rt, &addr, commit_req());
}

criterion_group!(benches, bench_nfs_pipeline);
criterion_main!(benches);
