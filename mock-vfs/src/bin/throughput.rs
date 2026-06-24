use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

use nfs_mamont::{handle_forever, Impl, ServerContext};

use mock_vfs::config::MockVfsConfig;
use mock_vfs::mock::{MockMount, MockVfs};
use mock_vfs::xdr::{build_rpc_frame, push_opaque, push_u32, push_u64};

// ── Arg parsing ─────────────────────────────────────────────────────────

struct Config {
    target: Option<String>,
    mode: Mode,
    block_size: usize,
    connections: u32,
    duration_s: f64,
    rwmixread: u8,
    endless: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Read,
    Write,
    Randrw,
}

fn parse_args() -> Config {
    let mut args = std::env::args().skip(1);
    let mut cfg = Config {
        target: None,
        mode: Mode::Read,
        block_size: 65536,
        connections: 4,
        duration_s: 5.0,
        rwmixread: 50,
        endless: false,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--target" => {
                cfg.target = Some(args.next().expect("--target requires an address"));
            }
            "--mode" => {
                let m = args.next().expect("--mode requires read|write|randrw");
                cfg.mode = match m.as_str() {
                    "read" => Mode::Read,
                    "write" => Mode::Write,
                    "randrw" => Mode::Randrw,
                    _ => panic!("Unknown mode {m}, expected read|write|randrw"),
                };
            }
            "--block-size" => {
                cfg.block_size = parse_size(&args.next().expect("--block-size requires a value"));
            }
            "--connections" => {
                cfg.connections = args
                    .next()
                    .expect("--connections requires a number")
                    .parse()
                    .expect("--connections must be a number");
            }
            "--duration" => {
                cfg.duration_s = args
                    .next()
                    .expect("--duration requires seconds")
                    .parse()
                    .expect("--duration must be a number");
            }
            "--rwmixread" => {
                cfg.rwmixread = args
                    .next()
                    .expect("--rwmixread requires percent")
                    .parse()
                    .expect("--rwmixread must be 0..100");
            }
            "--endless" => {
                cfg.endless = true;
            }
            "--help" | "-h" => {
                eprintln!(
                    "\
Usage: throughput [OPTIONS]

Options:
  --target <addr>       NFS server address (default: start local embedded server)
  --mode <mode>         Benchmark mode: read, write, randrw (default: read)
  --block-size <n>      Block size in bytes, supports K/M/G suffix (default: 65536)
  --connections <n>     Number of concurrent connections (default: 4)
  --duration <secs>     Test duration in seconds (default: 5.0)
  --rwmixread <percent> Read percentage for randrw mode (default: 50)
  --endless             Run until Ctrl+C (ignore --duration)
  --help, -h            Show this help and exit
"
                );
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {arg}");
                eprintln!("Use --help for usage");
                std::process::exit(1);
            }
        }
    }

    if let Ok(v) = std::env::var("MOCK_TARGET") {
        cfg.target = Some(v);
    }
    cfg
}

fn parse_size(s: &str) -> usize {
    let s = s.trim();
    let (num, mult) = if let Some(rest) = s.strip_suffix(|c| c == 'K' || c == 'k') {
        (rest, 1024)
    } else if let Some(rest) = s.strip_suffix(|c| c == 'M' || c == 'm') {
        (rest, 1024 * 1024)
    } else if let Some(rest) = s.strip_suffix(|c| c == 'G' || c == 'g') {
        (rest, 1024 * 1024 * 1024)
    } else {
        (s, 1)
    };
    num.parse::<usize>().unwrap_or_else(|_| panic!("Invalid block size: {s}")) * mult
}

// ── Request builders ───────────────────────────────────────────────────

const XID_BASE: u32 = 1000;

fn build_read_req(fh: &[u8; 8], offset: u64, count: u32, xid: u32) -> Vec<u8> {
    let mut a = Vec::new();
    push_opaque(&mut a, fh);
    push_u64(&mut a, offset);
    push_u32(&mut a, count);
    build_rpc_frame(6, xid, &a)
}

fn build_write_req(fh: &[u8; 8], offset: u64, data: &[u8], xid: u32) -> Vec<u8> {
    let mut a = Vec::new();
    push_opaque(&mut a, fh);
    push_u64(&mut a, offset);
    push_u32(&mut a, data.len() as u32);
    push_u32(&mut a, 2);
    push_opaque(&mut a, data);
    build_rpc_frame(7, xid, &a)
}

// ── Response reader ─────────────────────────────────────────────────────

async fn read_response(stream: &mut TcpStream, buf: &mut Vec<u8>) {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await.unwrap();
    let frag = u32::from_be_bytes(header);
    let len = (frag & 0x7FFF_FFFF) as usize;

    buf.resize(len, 0);
    stream.read_exact(buf).await.unwrap();
}

// ── Live stats reporter ─────────────────────────────────────────────────

struct StatsSnapshot {
    read_ops: u64,
    read_bytes: u64,
    write_ops: u64,
    write_bytes: u64,
}

impl StatsSnapshot {
    fn from_atomics(s: &ThreadStats) -> Self {
        Self {
            read_ops: s.read_ops.load(Ordering::Relaxed),
            read_bytes: s.read_bytes.load(Ordering::Relaxed),
            write_ops: s.write_ops.load(Ordering::Relaxed),
            write_bytes: s.write_bytes.load(Ordering::Relaxed),
        }
    }
}

async fn print_stats(stats: Arc<ThreadStats>, mut stop: watch::Receiver<bool>) {
    let start = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.tick().await; // skip first immediate tick

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = stop.wait_for(|v| *v) => break,
        }

        let cur = StatsSnapshot::from_atomics(&stats);
        let elapsed = start.elapsed().as_secs_f64();

        let total_ops = cur.read_ops + cur.write_ops;
        let total_bytes = cur.read_bytes + cur.write_bytes;
        let mb_s = total_bytes as f64 / elapsed / 1_000_000.0;

        eprint!(
            "\r[{:>4}s] read: {:>7} ops {:>8.1} MB/s | write: {:>7} ops {:>8.1} MB/s | total: {:>7} ops {:>8.1} MB/s  ",
            elapsed as u64,
            cur.read_ops,
            cur.read_bytes as f64 / elapsed / 1_000_000.0,
            cur.write_ops,
            cur.write_bytes as f64 / elapsed / 1_000_000.0,
            total_ops,
            mb_s,
        );
    }
    eprintln!();
}

// ── Worker task ─────────────────────────────────────────────────────────

struct ThreadStats {
    read_ops: AtomicU64,
    read_bytes: AtomicU64,
    write_ops: AtomicU64,
    write_bytes: AtomicU64,
}

async fn worker(
    conn_id: u32,
    addr: std::net::SocketAddr,
    cfg: Arc<Config>,
    stop: watch::Receiver<bool>,
    stats: Arc<ThreadStats>,
) {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    if let Err(e) = stream.set_nodelay(true) {
        eprintln!("[debug] set_nodelay failed on conn {conn_id}: {e}");
    }

    let fh: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];
    let write_buf = if cfg.mode == Mode::Write || cfg.mode == Mode::Randrw {
        Some(vec![0xABu8; cfg.block_size])
    } else {
        None
    };

    let mut rng = fastrand::Rng::new();
    let block_size_u32 = cfg.block_size as u32;
    let max_offset = 1024u64 * 1024 * 1024;

    let mut xid = XID_BASE + conn_id * 10000;
    let mut resp_buf = Vec::with_capacity(1024);

    while !*stop.borrow() {
        let is_read = match cfg.mode {
            Mode::Read => true,
            Mode::Write => false,
            Mode::Randrw => rng.u8(..100) < cfg.rwmixread,
        };

        let offset = rng.u64(..max_offset);
        xid += 1;

        if is_read {
            let req = build_read_req(&fh, offset, block_size_u32, xid);
            stream.write_all(&req).await.unwrap();
            read_response(&mut stream, &mut resp_buf).await;
            stats.read_ops.fetch_add(1, Ordering::Relaxed);
            stats.read_bytes.fetch_add(cfg.block_size as u64, Ordering::Relaxed);
        } else {
            let data = write_buf.as_deref().unwrap();
            let req = build_write_req(&fh, offset, data, xid);
            stream.write_all(&req).await.unwrap();
            read_response(&mut stream, &mut resp_buf).await;
            stats.write_ops.fetch_add(1, Ordering::Relaxed);
            stats.write_bytes.fetch_add(cfg.block_size as u64, Ordering::Relaxed);
        }
    }
}

// ── Local server startup ────────────────────────────────────────────────

async fn start_local_server() -> std::net::SocketAddr {
    let config = MockVfsConfig::default();
    let backend = Arc::new(MockVfs::new(config));
    let buf_size = NonZeroUsize::new(65536).unwrap();
    let buf_count = NonZeroUsize::new(1024).unwrap();
    let read_alloc = Arc::new(Impl::new(buf_size, buf_count));
    let write_alloc = Arc::new(Impl::new(buf_size, buf_count));
    let pool_size = NonZeroUsize::new(4).unwrap();
    let context = ServerContext::new(backend, read_alloc, write_alloc, pool_size);
    let mount_service = Arc::new(MockMount);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        handle_forever(listener, context, mount_service).await.unwrap();
    });

    for _ in 0..100 {
        if TcpStream::connect(addr).await.is_ok() {
            return addr;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Local server did not become ready within 1s");
}

// ── Main ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cfg = parse_args();

    let addr = if let Some(target) = &cfg.target {
        target
            .parse::<std::net::SocketAddr>()
            .expect("Invalid --target, expected ip:port (e.g. 192.168.1.100:2049)")
    } else {
        eprintln!("Local mode: starting embedded server");
        start_local_server().await
    };

    eprintln!(
        "Mode: {:?}, block: {}, connections: {}, {}",
        cfg.mode,
        format_size(cfg.block_size),
        cfg.connections,
        if cfg.endless { "endless".to_string() } else { format!("duration: {}s", cfg.duration_s) },
    );

    let stats = Arc::new(ThreadStats {
        read_ops: AtomicU64::new(0),
        read_bytes: AtomicU64::new(0),
        write_ops: AtomicU64::new(0),
        write_bytes: AtomicU64::new(0),
    });

    let (stop_tx, stop_rx) = watch::channel(false);

    let cfg = Arc::new(cfg);
    let mut handles = Vec::new();
    for i in 0..cfg.connections {
        let stats = Arc::clone(&stats);
        let stop_rx = stop_rx.clone();
        let cfg = Arc::clone(&cfg);
        handles.push(tokio::spawn(worker(i, addr, cfg, stop_rx, stats)));
    }

    let reporter_stop = stop_rx.clone();
    let reporter_stats = Arc::clone(&stats);
    let reporter = tokio::spawn(print_stats(reporter_stats, reporter_stop));

    let start = Instant::now();
    if cfg.endless {
        eprintln!("Running until Ctrl+C...");
        tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    } else {
        tokio::time::sleep(Duration::from_secs_f64(cfg.duration_s)).await;
    }
    stop_tx.send(true).unwrap();

    for h in handles {
        h.await.unwrap();
    }
    reporter.await.unwrap();

    let read_ops = stats.read_ops.load(Ordering::Relaxed);
    let read_bytes = stats.read_bytes.load(Ordering::Relaxed);
    let write_ops = stats.write_ops.load(Ordering::Relaxed);
    let write_bytes = stats.write_bytes.load(Ordering::Relaxed);

    let total_ops = read_ops + write_ops;
    let total_bytes = read_bytes + write_bytes;
    let dur = start.elapsed().as_secs_f64();
    let mb_per_s = total_bytes as f64 / dur / 1_000_000.0;
    let ops_per_s = total_ops as f64 / dur;

    println!();
    println!("=== Throughput Results ===");
    println!("Mode:        {:?}", cfg.mode);
    println!("Block size:  {}", cfg.block_size);
    println!("Connections: {}", cfg.connections);
    println!("Duration:    {dur:.1}s");
    println!();

    if read_ops > 0 {
        println!(
            "Reads:  {read_ops} ops, {} bytes ({})",
            read_bytes,
            format_size(read_bytes as usize)
        );
    }
    if write_ops > 0 {
        println!(
            "Writes: {write_ops} ops, {} bytes ({})",
            write_bytes,
            format_size(write_bytes as usize)
        );
    }
    println!();
    println!(
        "Total: {} in {dur:.1}s = {:.1} MB/s, {:.0} ops/s",
        format_size(total_bytes as usize),
        mb_per_s,
        ops_per_s,
    );
}

fn format_size(v: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut v = v as f64;
    for u in UNITS {
        if v < 1024.0 {
            return format!("{v:.1} {u}");
        }
        v /= 1024.0;
    }
    format!("{v:.1} TB")
}
