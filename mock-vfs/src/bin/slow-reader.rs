//! `slow-reader`: a deliberately misbehaving NFSv3 client.
//!
//! It performs a normal `MNT /` and `LOOKUP file_0` with the crate's client to learn a
//! file handle, then opens `--connections` more TCP connections and pipelines a
//! pre-encoded request into each one as fast as the server accepts the bytes, **without
//! ever reading a single reply**. A client that never drains its socket forces the
//! server's write side into back-pressure once the kernel send buffers fill; this tool
//! measures how many requests the server accepts before that happens.

use std::io;
use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use nfs_mamont::consts::nfsv3::{NfsProc, NFS_PROGRAM, NFS_VERSION};

use mock_vfs::cli::{self, Args};
use mock_vfs::client::{self, mount, nfs, Connection};

/// Procedure numbers used by the flood, derived from the typed [`NfsProc`] enum.
const NULL: u32 = NfsProc::Null as u32;
const GETATTR: u32 = NfsProc::GetAttr as u32;
const READ: u32 = NfsProc::Read as u32;

/// Number of copies of the record written per `write` call.
const BATCH_COPIES: usize = 256;
/// Timeout of a single `write`; a timeout means the server stopped reading (back-pressure).
const WRITE_TIMEOUT: Duration = Duration::from_millis(200);

const USAGE: &str = "\
slow-reader: a misbehaving NFSv3 client that floods a server and never reads replies

Usage: slow-reader [OPTIONS]

Options:
  --target <addr>        NFS server to flood (default: 127.0.0.1:2049)
  --op <op>              Request to pipeline: getattr, null or read (default: getattr)
  --block <bytes>        Transfer size of read requests, e.g. 1M (default: 1M)
  --duration <dur>       How long to flood, e.g. 20s (default: 20s)
  --connections <n>      Number of flooding TCP connections (default: 1)
  -h, --help             Show this help

The client mounts / and looks up file_0 over a normal connection, then opens
--connections more connections and writes pre-encoded requests as fast as the server
accepts them, never reading a reply, to exercise the server's write back-pressure.";

/// The request a flooding connection pipelines.
#[derive(Clone, Copy)]
enum SlowOp {
    Null,
    GetAttr,
    Read,
}

impl SlowOp {
    fn parse(name: &str) -> cli::Result<SlowOp> {
        match name {
            "null" => Ok(SlowOp::Null),
            "getattr" => Ok(SlowOp::GetAttr),
            "read" => Ok(SlowOp::Read),
            other => Err(format!("unknown --op '{other}', expected getattr, null or read")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            SlowOp::Null => "null",
            SlowOp::GetAttr => "getattr",
            SlowOp::Read => "read",
        }
    }
}

struct Options {
    target: SocketAddr,
    op: SlowOp,
    block: u64,
    duration: Duration,
    connections: usize,
}

enum Command {
    Run(Options),
    Help,
}

fn parse_args(mut args: Args) -> cli::Result<Command> {
    let mut target = SocketAddr::from(([127, 0, 0, 1], 2049));
    let mut op = SlowOp::GetAttr;
    let mut block: u64 = 1 << 20;
    let mut duration = Duration::from_secs(20);
    let mut connections = 1;

    while let Some(flag) = args.next_flag() {
        match flag.as_str() {
            "--target" => target = args.parse(&flag)?,
            "--op" => op = SlowOp::parse(&args.value(&flag)?)?,
            "--block" => block = args.parse_with(&flag, cli::parse_size)?,
            "--duration" => duration = args.parse_with(&flag, cli::parse_duration)?,
            "--connections" => {
                connections = args.parse_with(&flag, cli::parse_nonzero_usize)?.get()
            }
            "-h" | "--help" => return Ok(Command::Help),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    if block == 0 || block > u64::from(u32::MAX) {
        return Err(format!("--block {block} is out of range"));
    }

    Ok(Command::Run(Options { target, op, block, duration, connections }))
}

/// One flooding connection: writes the batch over and over, never reading a reply.
async fn flood(
    addr: SocketAddr,
    batch: Arc<Vec<u8>>,
    record_len: usize,
    accepted: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
) -> io::Result<()> {
    let stream = TcpStream::connect(addr).await?;
    let _ = stream.set_nodelay(true);
    let (reader, mut writer) = stream.into_split();
    // Keep the read half open but never read from it: this is what makes the client
    // "slow" and eventually fills the server's send buffer.
    let _reader = reader;

    let mut pos = 0usize;
    // Bytes written toward completing the current record, carried across writes.
    let mut carry = 0usize;
    while !stop.load(Ordering::Relaxed) {
        // `write` (not `write_all`) is cancel safe, so a timeout leaves nothing
        // half-written: we can simply retry. A timeout is the server pushing back.
        match tokio::time::timeout(WRITE_TIMEOUT, writer.write(&batch[pos..])).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                pos += n;
                carry += n;
                if carry >= record_len {
                    accepted.fetch_add((carry / record_len) as u64, Ordering::Relaxed);
                    carry %= record_len;
                }
                if pos >= batch.len() {
                    pos = 0;
                }
            }
            Ok(Err(err)) => return Err(err),
            Err(_elapsed) => {}
        }
    }

    Ok(())
}

async fn run(options: Options) -> Result<(), String> {
    let mut conn = Connection::connect(options.target)
        .await
        .map_err(|err| format!("cannot connect to {}: {err}", options.target))?;
    let root = mount::mount(&mut conn, "/").await.map_err(|err| format!("MNT / failed: {err}"))?;
    let file = nfs::lookup(&mut conn, &root, "file_0")
        .await
        .map_err(|err| format!("LOOKUP file_0 failed: {err}"))?
        .handle;
    drop(conn);

    let record = match options.op {
        SlowOp::Null => client::call_record(NFS_PROGRAM, NFS_VERSION, NULL, 1, &[]),
        SlowOp::GetAttr => {
            client::call_record(NFS_PROGRAM, NFS_VERSION, GETATTR, 1, &nfs::args::getattr(&file))
        }
        SlowOp::Read => client::call_record(
            NFS_PROGRAM,
            NFS_VERSION,
            READ,
            1,
            &nfs::args::read(&file, 0, options.block as u32),
        ),
    };
    let record_len = record.len();
    let mut buf = Vec::with_capacity(record_len * BATCH_COPIES);
    for _ in 0..BATCH_COPIES {
        buf.extend_from_slice(&record);
    }
    let batch = Arc::new(buf);
    let accepted = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));

    eprintln!(
        "slow-reader: {} connection(s) flooding {} with {} requests of {record_len} B, \
         never reading replies, for {}",
        options.connections,
        options.target,
        options.op.name(),
        cli::format_duration(options.duration),
    );

    let mut tasks = Vec::with_capacity(options.connections);
    for _ in 0..options.connections {
        tasks.push(tokio::spawn(flood(
            options.target,
            Arc::clone(&batch),
            record_len,
            Arc::clone(&accepted),
            Arc::clone(&stop),
        )));
    }

    let start = Instant::now();
    let total = tokio::time::sleep(options.duration);
    tokio::pin!(total);
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    ticker.tick().await; // the first tick is immediate; skip it
    loop {
        tokio::select! {
            _ = &mut total => break,
            _ = ticker.tick() => {
                println!(
                    "t={}s accepted_requests={}",
                    start.elapsed().as_secs(),
                    accepted.load(Ordering::Relaxed),
                );
            }
        }
    }

    stop.store(true, Ordering::Relaxed);
    for task in tasks {
        let _ = task.await;
    }
    println!("total_accepted_requests={}", accepted.load(Ordering::Relaxed));

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let options = match parse_args(Args::from_env()) {
        Ok(Command::Run(options)) => options,
        Ok(Command::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    cli::init_tracing("warn,nfs_mamont::task::connection::read=off");

    match run(options).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}
