//! Runs real NFS READ/GETATTR traffic through the server, including error replies.
//! Usage: cargo run --release --features metrics --example metrics_smoke -- 30
//! Set NFS_METRICS_BACKEND_DELAY_US=0 for an instrumentation overhead comparison.

use std::io;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use nfs_mamont::auth::Credential;
use nfs_mamont::service::{mount::MountService, nlm::NlmService};
use nfs_mamont::vfs::{self, file};
use nfs_mamont::{Buffer, Impl, ServerContext, Slice};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct DemoBackend {
    delay: Duration,
}

fn wcc() -> vfs::WccData {
    vfs::WccData { before: None, after: None }
}

// Only READ is implemented. Other procedures return normal NFS error replies.
macro_rules! unsupported {
    ($module:ident, $trait:ident, $method:ident $(, $field:ident: $value:expr)* $(,)?) => {
        impl vfs::$module::$trait for DemoBackend {
            async fn $method(&self, _: vfs::$module::Args, _: &Credential) -> Result<vfs::$module::Success, vfs::$module::Fail> {
                Err(vfs::$module::Fail { error: vfs::Error::NotSupported, $($field: $value,)* })
            }
        }
    };
}

unsupported!(get_attr, GetAttr, get_attr);
unsupported!(set_attr, SetAttr, set_attr, wcc_data: wcc());
unsupported!(lookup, Lookup, lookup, dir_attr: None);
unsupported!(access, Access, access, object_attr: None);
unsupported!(read_link, ReadLink, read_link, symlink_attr: None);
unsupported!(create, Create, create, wcc_data: wcc());
unsupported!(mk_dir, MkDir, mk_dir, dir_wcc: wcc());
unsupported!(symlink, Symlink, symlink, dir_wcc: wcc());
unsupported!(mk_node, MkNode, mk_node, dir_wcc: wcc());
unsupported!(remove, Remove, remove, dir_wcc: wcc());
unsupported!(rm_dir, RmDir, rm_dir, dir_wcc: wcc());
unsupported!(rename, Rename, rename, from_dir_wcc: wcc(), to_dir_wcc: wcc());
unsupported!(link, Link, link, file_attr: None, dir_wcc: wcc());
unsupported!(read_dir, ReadDir, read_dir, dir_attr: None);
unsupported!(read_dir_plus, ReadDirPlus, read_dir_plus, dir_attr: None);
unsupported!(fs_stat, FsStat, fs_stat, root_attr: None);
unsupported!(fs_info, FsInfo, fs_info, root_attr: None);
unsupported!(path_conf, PathConf, path_conf, file_attr: None);
unsupported!(commit, Commit, commit, file_wcc: wcc());

impl vfs::read::Read<Slice> for DemoBackend {
    async fn read(
        &self,
        _: vfs::read::Args,
        mut data: Slice,
        _: &Credential,
    ) -> Result<vfs::read::Success<Slice>, vfs::read::Fail> {
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        for chunk in data.chunks_mut() {
            chunk.fill(0x42);
        }
        Ok(vfs::read::Success {
            head: vfs::read::SuccessHeader { file_attr: None, count: data.len() as u32, eof: true },
            data,
        })
    }
}

impl vfs::write::Write<Slice> for DemoBackend {
    async fn write(
        &self,
        _: vfs::write::Args<Slice>,
        _: &Credential,
    ) -> Result<vfs::write::Success, vfs::write::Fail> {
        Err(vfs::write::Fail { error: vfs::Error::NotSupported, wcc_data: wcc() })
    }
}

fn request(xid: u32, handle: &file::Handle, read: bool) -> Vec<u8> {
    let mut body = Vec::new();
    // RPC CALL, version 2, NFSv3, AUTH_NONE credential and verifier.
    for word in [xid, 0, 2, 100003, 3, if read { 6 } else { 1 }, 0, 0, 0, 0, handle.0.len() as u32]
    {
        body.extend_from_slice(&word.to_be_bytes());
    }
    body.extend_from_slice(&handle.0);
    body.resize(body.len().next_multiple_of(4), 0);
    if read {
        body.extend_from_slice(&0_u64.to_be_bytes());
        body.extend_from_slice(&4096_u32.to_be_bytes());
    }
    let mut record = ((body.len() as u32) | 0x8000_0000).to_be_bytes().to_vec();
    record.extend(body);
    record
}

async fn load(
    address: std::net::SocketAddr,
    handle: file::Handle,
    stop: Arc<AtomicBool>,
    completed: Arc<AtomicU64>,
) -> io::Result<()> {
    let mut socket = TcpStream::connect(address).await?;
    socket.set_nodelay(true)?;
    while !stop.load(Ordering::Relaxed) {
        // Pipeline enough requests to exhaust the four-block allocator and occupy
        // both VFS workers. GETATTR errors exercise the same completion metrics.
        for xid in 0..16 {
            socket.write_all(&request(xid, &handle, xid % 4 != 0)).await?;
        }
        let mut seen = 0_u16;
        for _ in 0..16 {
            let marker = socket.read_u32().await?;
            if marker & 0x8000_0000 == 0 {
                return Err(io::Error::other("unexpected fragmented reply"));
            }
            let mut reply = vec![0; (marker & 0x7fff_ffff) as usize];
            socket.read_exact(&mut reply).await?;
            if reply.len() < 28
                || reply[4..8] != 1_u32.to_be_bytes()
                || reply[20..24] != 0_u32.to_be_bytes()
            {
                return Err(io::Error::other(format!("unexpected RPC reply: {reply:?}")));
            }
            let replied_xid = u32::from_be_bytes(reply[..4].try_into().unwrap());
            // Responses may arrive out of order because VFS workers run concurrently.
            let expected = if replied_xid % 4 == 0 { vfs::Error::NotSupported as u32 } else { 0 };
            if reply[24..28] != expected.to_be_bytes() || replied_xid >= 16 {
                return Err(io::Error::other("unexpected NFS status or transaction ID"));
            }
            let bit = 1_u16 << replied_xid;
            if seen & bit != 0 {
                return Err(io::Error::other("duplicate RPC reply"));
            }
            seen |= bit;
            completed.fetch_add(1, Ordering::Relaxed);
        }
    }
    socket.shutdown().await
}

#[tokio::main(worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if std::env::var_os("RUST_LOG").is_some() {
        nfs_mamont::init_tracing();
    }
    #[cfg(feature = "metrics")]
    nfs_mamont::install_prometheus_exporter(([0, 0, 0, 0], 9000).into())?;
    let seconds = std::env::args().nth(1).map(|value| value.parse()).transpose()?.unwrap_or(30);
    let delay = std::env::var("NFS_METRICS_BACKEND_DELAY_US")
        .ok()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(25_000);
    let allocator =
        Arc::new(Impl::new(NonZeroUsize::new(4096).unwrap(), NonZeroUsize::new(4).unwrap()));
    let context = ServerContext::new(allocator.clone(), NonZeroUsize::new(10).unwrap());
    let backend =
        context.add_backend(Arc::new(DemoBackend { delay: Duration::from_micros(delay) })).unwrap();
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(nfs_mamont::handle_forever(
        listener,
        context,
        Arc::new(MountService::with_exports(vec![])),
        Arc::new(NlmService::new()),
    ));
    let stop = Arc::new(AtomicBool::new(false));
    let completed = Arc::new(AtomicU64::new(0));
    let started = Instant::now();
    let mut clients = Vec::new();
    for _ in 0..4 {
        clients.push(tokio::spawn(load(
            address,
            file::Handle::new(backend, [1; file::PAYLOAD_SIZE]),
            stop.clone(),
            completed.clone(),
        )));
    }
    eprintln!(
        "Real NFS workload at {address}; metrics feature: {}; duration: {seconds}s",
        cfg!(feature = "metrics")
    );
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(seconds)) => {},
        signal = tokio::signal::ctrl_c() => { signal?; },
    }
    stop.store(true, Ordering::Relaxed);
    for client in clients {
        client.await??;
    }
    let elapsed = started.elapsed().as_secs_f64();
    eprintln!(
        "Completed {} RPCs in {elapsed:.3}s ({:.0} RPC/s)",
        completed.load(Ordering::Relaxed),
        completed.load(Ordering::Relaxed) as f64 / elapsed
    );
    server.abort();
    let _ = server.await;
    // Let connection tasks consume EOF and drop their instrumentation before flush.
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    #[cfg(feature = "metrics")]
    {
        nfs_mamont::flush_metrics();
        eprintln!("Workload drained; keeping exporter alive for two final scrapes (11s)");
        tokio::time::sleep(Duration::from_secs(11)).await;
    }
    // Keep this ownership explicit: the allocator remains alive through the final scrape.
    drop(allocator);
    Ok(())
}
