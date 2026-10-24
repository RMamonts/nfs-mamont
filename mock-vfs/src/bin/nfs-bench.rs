//! `nfs-bench`: load generator for NFSv3 servers.
//!
//! Runs one or more phases. In every phase `--connections` TCP connections keep
//! `--depth` requests of one operation in flight each; at the end the phase reports
//! throughput and latency percentiles. Without `--target` an embedded mock server is
//! started, configured through the server flags, so `nfs-bench` alone measures the
//! NFS Mamont pipeline against a backend with a known response time.
//!
//! Requests are pipelined per connection and matched to replies by transaction id,
//! so out-of-order completion (different backend latencies, jitter) is handled.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::Semaphore;

use nfs_mamont::consts::nfsv3::{
    ACCESS, COMMIT, CREATE, GETATTR, LOOKUP, NFS_PROGRAM, NFS_VERSION, NULL, READ, READDIR,
    READDIRPLUS, REMOVE, WRITE,
};

use mock_vfs::cli::{self, Args};
use mock_vfs::client::{self, mount, nfs, Connection};
use mock_vfs::hist::Histogram;
use mock_vfs::latency::splitmix64;
use mock_vfs::server::{Server, ServerConfig};

const USAGE_HEAD: &str = "\
nfs-bench: NFSv3 load generator reporting throughput and latency percentiles

Usage: nfs-bench [OPTIONS]

Workload:
  --target <addr>        NFS server to benchmark (default: start an embedded mock server)
  --export <path>        Export to mount (default: /)
  --file <name>          File inside the export to read and write (default: file_0)
  --op <list>            Comma-separated phases, each `op[:block]`, where op is one of
                         read, write, randrw, getattr, lookup, access, readdir,
                         readdirplus, create, remove, commit, null (default: read)
  --block <bytes>        Transfer size of read/write/randrw without `:block` (default: 64K)
  --rwmix <percent>      Share of reads in randrw (default: 70)
  --connections <n>      TCP connections (default: 4)
  --depth <n>            Requests kept in flight per connection (default: 1)
  --duration <dur>       Length of every phase, 0 = until Ctrl+C (default: 5s)
  --warmup <dur>         Start of every phase excluded from the statistics (default: 0)
  --seed <n>             Seed of the offset generator (default: 1)
  --quiet                Do not print the live progress line
  -h, --help             Show this help

Embedded server (ignored with --target):";

const USAGE_TAIL: &str = "\
Offsets of read/write are random multiples of the block size inside the file. Latency
percentiles have a resolution of about 3 %. Results go to stdout, progress to stderr.";

/// Transaction id of the NULL call that closes a connection's request stream.
const SENTINEL_XID: u32 = 0;
/// `count`/`maxcount` of READDIR and READDIRPLUS requests.
const READDIR_COUNT: u32 = 32 * 1024;
const MIB: f64 = 1024.0 * 1024.0;
const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

fn usage() -> String {
    format!("{USAGE_HEAD}\n{}\n\n{USAGE_TAIL}", cli::SERVER_FLAGS_HELP)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Null,
    GetAttr,
    Lookup,
    Access,
    ReadDir,
    ReadDirPlus,
    Read,
    Write,
    RandRw,
    Create,
    Remove,
    Commit,
}

impl Op {
    const ALL: [(&'static str, Op); 12] = [
        ("null", Op::Null),
        ("getattr", Op::GetAttr),
        ("lookup", Op::Lookup),
        ("access", Op::Access),
        ("readdir", Op::ReadDir),
        ("readdirplus", Op::ReadDirPlus),
        ("read", Op::Read),
        ("write", Op::Write),
        ("randrw", Op::RandRw),
        ("create", Op::Create),
        ("remove", Op::Remove),
        ("commit", Op::Commit),
    ];

    fn parse(name: &str) -> Option<Op> {
        Self::ALL.iter().find(|(known, _)| *known == name).map(|(_, op)| *op)
    }

    fn name(self) -> &'static str {
        Self::ALL.iter().find(|(_, op)| *op == self).map_or("?", |(name, _)| name)
    }

    fn transfers_data(self) -> bool {
        matches!(self, Op::Read | Op::Write | Op::RandRw)
    }
}

#[derive(Clone, Debug)]
struct Phase {
    op: Op,
    block: u64,
}

impl Phase {
    fn label(&self) -> String {
        if self.op.transfers_data() {
            format!("{}:{}", self.op.name(), cli::format_size_short(self.block))
        } else {
            self.op.name().to_string()
        }
    }
}

struct Options {
    target: Option<SocketAddr>,
    server: ServerConfig,
    file: String,
    phases: Vec<Phase>,
    rwmix: u8,
    connections: usize,
    depth: usize,
    duration: Duration,
    warmup: Duration,
    seed: u64,
    quiet: bool,
}

enum Command {
    Run(Box<Options>),
    Help,
}

fn parse_positive(text: &str) -> cli::Result<usize> {
    let value = cli::parse_size(text)?;
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("'{text}' must be a positive number"))
}

fn parse_phases(list: &str, default_block: u64) -> cli::Result<Vec<Phase>> {
    let mut phases = Vec::new();
    for item in list.split(',').map(str::trim).filter(|item| !item.is_empty()) {
        let (name, block) = match item.split_once(':') {
            Some((name, block)) => (name, Some(block)),
            None => (item, None),
        };
        let op = Op::parse(name).ok_or_else(|| format!("unknown operation '{name}'"))?;
        let block = match block {
            Some(block) => cli::parse_size(block)?,
            None => default_block,
        };
        if block == 0 || block > u64::from(u32::MAX) {
            return Err(format!("block size {block} of '{item}' is out of range"));
        }
        phases.push(Phase { op, block });
    }
    if phases.is_empty() {
        return Err("--op lists no operation".to_string());
    }

    Ok(phases)
}

fn parse_args(mut args: Args) -> cli::Result<Command> {
    let mut options = Options {
        target: None,
        server: ServerConfig::default(),
        file: "file_0".to_string(),
        phases: Vec::new(),
        rwmix: 70,
        connections: 4,
        depth: 1,
        duration: Duration::from_secs(5),
        warmup: Duration::ZERO,
        seed: 1,
        quiet: false,
    };
    let mut ops = "read".to_string();
    let mut block = 64 << 10;

    while let Some(flag) = args.next_flag() {
        match flag.as_str() {
            "--target" => options.target = Some(args.parse(&flag)?),
            "--file" => options.file = args.value(&flag)?,
            "--op" => ops = args.value(&flag)?,
            "--block" => block = args.parse_with(&flag, cli::parse_size)?,
            "--rwmix" => options.rwmix = args.parse(&flag)?,
            "--connections" => options.connections = args.parse_with(&flag, parse_positive)?,
            "--depth" => options.depth = args.parse_with(&flag, parse_positive)?,
            "--duration" => options.duration = args.parse_with(&flag, cli::parse_duration)?,
            "--warmup" => options.warmup = args.parse_with(&flag, cli::parse_duration)?,
            "--seed" => options.seed = args.parse(&flag)?,
            "--quiet" => options.quiet = true,
            "-h" | "--help" => return Ok(Command::Help),
            other => {
                if !cli::apply_server_flag(&mut options.server, other, &mut args)? {
                    return Err(format!("unknown argument '{other}'"));
                }
            }
        }
    }

    options.phases = parse_phases(&ops, block)?;
    if options.rwmix > 100 {
        return Err("--rwmix must be between 0 and 100".to_string());
    }
    if !options.duration.is_zero() && options.warmup >= options.duration {
        return Err("--warmup must be shorter than --duration".to_string());
    }
    if options.target.is_none() {
        options.server.validate().map_err(|err| err.to_string())?;
    }

    Ok(Command::Run(Box::new(options)))
}

/// xorshift64*: fast, good enough for picking offsets.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(splitmix64(seed) | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }
}

/// What the setup connection learned about the server.
struct Target {
    addr: SocketAddr,
    root: Vec<u8>,
    file: Vec<u8>,
    file_size: u64,
    rtmax: u32,
    wtmax: u32,
}

async fn prepare(addr: SocketAddr, options: &Options) -> Result<Target, String> {
    let export = &options.server.export;
    let mut conn = Connection::connect(addr)
        .await
        .map_err(|err| format!("cannot connect to {addr}: {err}"))?;
    let root = mount::mount(&mut conn, export)
        .await
        .map_err(|err| format!("MNT {export} failed: {err}"))?;
    let file = nfs::lookup(&mut conn, &root, &options.file)
        .await
        .map_err(|err| format!("LOOKUP {} failed: {err}", options.file))?;
    let attr = match file.attr {
        Some(attr) => attr,
        None => nfs::getattr(&mut conn, &file.handle)
            .await
            .map_err(|err| format!("GETATTR {} failed: {err}", options.file))?,
    };
    if attr.file_type != nfs::TYPE_REGULAR {
        return Err(format!("{} is not a regular file", options.file));
    }
    let info =
        nfs::fsinfo(&mut conn, &root).await.map_err(|err| format!("FSINFO failed: {err}"))?;

    Ok(Target {
        addr,
        root,
        file: file.handle,
        file_size: attr.size,
        rtmax: info.rtmax,
        wtmax: info.wtmax,
    })
}

/// State shared by the workers of one phase.
struct Shared {
    addr: SocketAddr,
    root: Vec<u8>,
    file: Vec<u8>,
    file_name: String,
    file_size: u64,
    phase: Phase,
    rwmix: u8,
    depth: usize,
    /// Set when the phase is over: workers stop issuing requests and drain.
    stop: AtomicBool,
    /// Replies received before this instant belong to the warm-up.
    measure_from: Instant,
    live_ops: AtomicU64,
    live_bytes: AtomicU64,
}

/// A pre-encoded request; only the xid and, for data operations, the offset change.
struct Template {
    record: Vec<u8>,
    offset_at: Option<usize>,
}

impl Template {
    fn new(procedure: u32, args: &[u8], offset_at: Option<usize>) -> Self {
        Self {
            record: client::call_record(NFS_PROGRAM, NFS_VERSION, procedure, 0, args),
            offset_at,
        }
    }

    fn fill(&mut self, xid: u32, offset: u64) -> &[u8] {
        client::set_xid(&mut self.record, xid);
        if let Some(at) = self.offset_at {
            self.record[at..at + 8].copy_from_slice(&offset.to_be_bytes());
        }
        &self.record
    }
}

/// Request templates of one worker.
struct Requests {
    read: Template,
    write: Template,
    other: Template,
    block: u64,
    blocks: u64,
}

impl Requests {
    fn new(shared: &Shared, worker: usize) -> Self {
        let (fh, root, block) = (&shared.file, &shared.root, shared.phase.block);
        let offset_at = client::ARGS_OFFSET + nfs::args::offset_position(fh.len());
        let read = Template::new(READ, &nfs::args::read(fh, 0, block as u32), Some(offset_at));
        let payload = match shared.phase.op {
            Op::Write | Op::RandRw => vec![0xA5u8; block as usize],
            _ => Vec::new(),
        };
        let write = Template::new(
            WRITE,
            &nfs::args::write(fh, 0, &payload, nfs::UNSTABLE),
            Some(offset_at),
        );
        let name = format!("bench_{worker}");
        let other = match shared.phase.op {
            Op::GetAttr => Template::new(GETATTR, &nfs::args::getattr(fh), None),
            Op::Lookup => Template::new(LOOKUP, &nfs::args::lookup(root, &shared.file_name), None),
            Op::Access => Template::new(ACCESS, &nfs::args::access(fh, nfs::ACCESS_ALL), None),
            Op::ReadDir => {
                Template::new(READDIR, &nfs::args::readdir(root, 0, &[0; 8], READDIR_COUNT), None)
            }
            Op::ReadDirPlus => Template::new(
                READDIRPLUS,
                &nfs::args::readdirplus(root, 0, &[0; 8], READDIR_COUNT, READDIR_COUNT * 2),
                None,
            ),
            Op::Create => Template::new(CREATE, &nfs::args::create(root, &name), None),
            Op::Remove => Template::new(REMOVE, &nfs::args::remove(root, &name), None),
            Op::Commit => Template::new(COMMIT, &nfs::args::commit(fh, 0, 0), None),
            Op::Null | Op::Read | Op::Write | Op::RandRw => Template::new(NULL, &[], None),
        };

        Self { read, write, other, block, blocks: shared.file_size / block }
    }

    /// Encodes the next request.
    ///
    /// # Returns
    ///
    /// The operation actually issued (`randrw` resolves to `read` or `write`) and
    /// the record to send.
    fn next(&mut self, op: Op, rwmix: u8, rng: &mut Rng, xid: u32) -> (Op, &[u8]) {
        let op = match op {
            Op::RandRw if rng.below(100) < u64::from(rwmix) => Op::Read,
            Op::RandRw => Op::Write,
            other => other,
        };
        match op {
            Op::Read | Op::Write => {
                let offset = if self.blocks > 0 { rng.below(self.blocks) * self.block } else { 0 };
                let template = if op == Op::Read { &mut self.read } else { &mut self.write };
                (op, template.fill(xid, offset))
            }
            _ => (op, self.other.fill(xid, 0)),
        }
    }
}

#[derive(Default)]
struct WorkerStats {
    hist: Histogram,
    bytes: u64,
    errors: u64,
    first_error: Option<String>,
}

struct Inflight {
    start: Instant,
    op: Op,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Bytes transferred by a successful reply, or the error it carries.
fn account(op: Op, body: &[u8]) -> Result<u64, client::Error> {
    match op {
        Op::Read => nfs::parse_read(body).map(|read| u64::from(read.count)),
        Op::Write => nfs::parse_write(body).map(|write| u64::from(write.count)),
        Op::Null => Ok(0),
        _ => nfs::status(body).map(|_| 0),
    }
}

fn micros(duration: Duration) -> u64 {
    duration.as_micros().min(u128::from(u64::MAX)) as u64
}

/// One connection: a sender keeping `depth` requests in flight and a receiver
/// matching replies to requests.
async fn worker(
    index: usize,
    shared: Arc<Shared>,
    seed: u64,
) -> Result<WorkerStats, client::Error> {
    let stream = TcpStream::connect(shared.addr).await?;
    let _ = stream.set_nodelay(true);
    let (mut reader, mut writer) = stream.into_split();

    let mut requests = Requests::new(&shared, index);
    let mut rng = Rng::new(seed);
    let inflight: Mutex<HashMap<u32, Inflight>> = Mutex::new(HashMap::new());
    let slots = Semaphore::new(shared.depth);

    let send = async {
        let mut xid: u32 = SENTINEL_XID + 1;
        while !shared.stop.load(Ordering::Relaxed) {
            slots.acquire().await.expect("the semaphore is never closed").forget();
            let (op, record) = requests.next(shared.phase.op, shared.rwmix, &mut rng, xid);
            lock(&inflight).insert(xid, Inflight { start: Instant::now(), op });
            writer.write_all(record).await?;
            xid = xid.wrapping_add(1);
            if xid == SENTINEL_XID {
                xid += 1;
            }
        }
        // The sentinel tells the receiver that no request follows: once everything
        // in flight is answered it can stop. NULL bypasses the VFS queue, so the
        // sentinel may be answered before earlier requests, which is why the
        // receiver also waits for the in-flight set to drain.
        let sentinel = client::call_record(NFS_PROGRAM, NFS_VERSION, NULL, SENTINEL_XID, &[]);
        writer.write_all(&sentinel).await?;

        Ok::<(), client::Error>(())
    };

    let receive = async {
        let mut stats = WorkerStats::default();
        let mut buf = Vec::with_capacity(64 * 1024);
        let mut sentinel_seen = false;
        loop {
            if sentinel_seen && lock(&inflight).is_empty() {
                return Ok::<WorkerStats, client::Error>(stats);
            }
            client::read_record(&mut reader, &mut buf).await?;
            let reply = client::parse_reply(&buf)?;
            if reply.xid == SENTINEL_XID {
                sentinel_seen = true;
                continue;
            }
            let Some(request) = lock(&inflight).remove(&reply.xid) else {
                return Err(client::Error::Malformed("reply to an unknown xid"));
            };
            let now = Instant::now();
            slots.add_permits(1);

            let outcome = reply
                .status
                .map_err(client::Error::Rpc)
                .and_then(|body| account(request.op, &buf[body..]));
            let bytes = match outcome {
                Ok(bytes) => bytes,
                Err(err) => {
                    stats.errors += 1;
                    if stats.first_error.is_none() {
                        stats.first_error = Some(err.to_string());
                    }
                    0
                }
            };
            shared.live_ops.fetch_add(1, Ordering::Relaxed);
            shared.live_bytes.fetch_add(bytes, Ordering::Relaxed);
            if now >= shared.measure_from {
                stats.hist.record(micros(now.saturating_duration_since(request.start)));
                stats.bytes += bytes;
            }
        }
    };

    let ((), stats) = tokio::try_join!(send, receive)?;
    Ok(stats)
}

async fn progress(shared: Arc<Shared>, label: String, start: Instant) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.tick().await;
    let (mut last_ops, mut last_bytes, mut last_at) = (0u64, 0u64, start);
    loop {
        interval.tick().await;
        let now = Instant::now();
        let ops = shared.live_ops.load(Ordering::Relaxed);
        let bytes = shared.live_bytes.load(Ordering::Relaxed);
        let secs = now.saturating_duration_since(last_at).as_secs_f64().max(1e-9);
        eprint!(
            "\r[{:>6.1}s] {:<14} {:>10.0} ops/s {:>9.1} MiB/s   ",
            now.saturating_duration_since(start).as_secs_f64(),
            label,
            (ops - last_ops) as f64 / secs,
            (bytes - last_bytes) as f64 / secs / MIB,
        );
        (last_ops, last_bytes, last_at) = (ops, bytes, now);
    }
}

/// Waits for the end of a phase.
///
/// # Returns
///
/// `true` when the phase was interrupted with `Ctrl+C`.
async fn wait_for_end(duration: Duration) -> bool {
    if duration.is_zero() {
        let _ = tokio::signal::ctrl_c().await;
        return true;
    }
    tokio::select! {
        _ = tokio::time::sleep(duration) => false,
        _ = tokio::signal::ctrl_c() => true,
    }
}

struct PhaseResult {
    label: String,
    elapsed: Duration,
    hist: Histogram,
    bytes: u64,
    errors: u64,
    first_error: Option<String>,
    failed_connections: usize,
    interrupted: bool,
}

async fn run_phase(
    phase: &Phase,
    target: &Target,
    options: &Options,
) -> Result<PhaseResult, String> {
    let start = Instant::now();
    let shared = Arc::new(Shared {
        addr: target.addr,
        root: target.root.clone(),
        file: target.file.clone(),
        file_name: options.file.clone(),
        file_size: target.file_size,
        phase: phase.clone(),
        rwmix: options.rwmix,
        depth: options.depth,
        stop: AtomicBool::new(false),
        measure_from: start + options.warmup,
        live_ops: AtomicU64::new(0),
        live_bytes: AtomicU64::new(0),
    });

    let workers: Vec<_> = (0..options.connections)
        .map(|index| {
            let seed = options.seed ^ (index as u64 + 1).wrapping_mul(GOLDEN_GAMMA);
            tokio::spawn(worker(index, Arc::clone(&shared), seed))
        })
        .collect();
    let reporter =
        (!options.quiet).then(|| tokio::spawn(progress(Arc::clone(&shared), phase.label(), start)));

    let interrupted = wait_for_end(options.duration).await;
    shared.stop.store(true, Ordering::Relaxed);

    let mut result = PhaseResult {
        label: phase.label(),
        elapsed: Duration::ZERO,
        hist: Histogram::new(),
        bytes: 0,
        errors: 0,
        first_error: None,
        failed_connections: 0,
        interrupted,
    };
    for handle in workers {
        match handle.await {
            Ok(Ok(stats)) => {
                result.hist.merge(&stats.hist);
                result.bytes += stats.bytes;
                result.errors += stats.errors;
                if result.first_error.is_none() {
                    result.first_error = stats.first_error;
                }
            }
            Ok(Err(err)) => {
                result.failed_connections += 1;
                result.first_error.get_or_insert_with(|| err.to_string());
            }
            Err(err) => {
                result.failed_connections += 1;
                result.first_error.get_or_insert_with(|| err.to_string());
            }
        }
    }
    result.elapsed = Instant::now().saturating_duration_since(shared.measure_from);

    if let Some(reporter) = reporter {
        reporter.abort();
        eprint!("\r{:<70}\r", "");
    }
    if result.failed_connections == options.connections {
        return Err(format!(
            "every connection of phase {} failed: {}",
            result.label,
            result.first_error.unwrap_or_default()
        ));
    }

    Ok(result)
}

fn print_header() {
    println!(
        "{:<14} {:>10} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>7}",
        "op", "ops/s", "MiB/s", "avg", "p50", "p99", "p99.9", "max", "errors"
    );
}

fn print_row(result: &PhaseResult) {
    let secs = result.elapsed.as_secs_f64().max(1e-9);
    let latency =
        |value: Option<u64>| value.map_or("-".to_string(), |us| cli::format_micros(us as f64));
    println!(
        "{:<14} {:>10.0} {:>9.1} {:>9} {:>9} {:>9} {:>9} {:>9} {:>7}",
        result.label,
        result.hist.count() as f64 / secs,
        result.bytes as f64 / secs / MIB,
        result.hist.mean().map_or("-".to_string(), cli::format_micros),
        latency(result.hist.percentile(50.0)),
        latency(result.hist.percentile(99.0)),
        latency(result.hist.percentile(99.9)),
        latency(result.hist.max()),
        result.errors,
    );
    if result.failed_connections > 0 {
        eprintln!("  {} connection(s) failed during {}", result.failed_connections, result.label);
    }
    if let Some(err) = &result.first_error {
        eprintln!("  first error of {}: {err}", result.label);
    }
}

async fn run(options: Options) -> Result<(), String> {
    let (server, addr) = match options.target {
        Some(addr) => (None, addr),
        None => {
            let server = Server::start(options.server.clone())
                .await
                .map_err(|err| format!("cannot start the embedded server: {err}"))?;
            let addr = server.addr();
            (Some(server), addr)
        }
    };

    let target = prepare(addr, &options).await?;
    let duration = if options.duration.is_zero() {
        "until Ctrl+C".to_string()
    } else {
        format!("{} per phase", cli::format_duration(options.duration))
    };
    eprintln!(
        "target {addr}{}, export {}, file {} ({}), {} connection(s) x {} in flight, {}{}",
        if server.is_some() { " (embedded mock server)" } else { "" },
        options.server.export,
        options.file,
        cli::format_size(target.file_size),
        options.connections,
        options.depth,
        duration,
        if options.warmup.is_zero() {
            String::new()
        } else {
            format!(" after {} of warm-up", cli::format_duration(options.warmup))
        },
    );
    if server.is_some() {
        let latency = options.server.vfs.latency;
        eprintln!(
            "backend latency {} ± {}, bandwidth {}, {} VFS workers",
            cli::format_duration(latency.base),
            cli::format_duration(latency.jitter),
            latency
                .bandwidth
                .map_or("unlimited".to_string(), |b| format!("{}/s", cli::format_size(b.get()))),
            options.server.concurrency,
        );
    }
    for phase in &options.phases {
        if phase.op.transfers_data() && phase.block > u64::from(target.rtmax.min(target.wtmax)) {
            eprintln!(
                "warning: block {} of {} exceeds the server limits (rtmax {}, wtmax {}); \
                 expect short transfers or errors",
                cli::format_size(phase.block),
                phase.label(),
                cli::format_size(u64::from(target.rtmax)),
                cli::format_size(u64::from(target.wtmax)),
            );
        }
    }

    print_header();
    for phase in &options.phases {
        let result = run_phase(phase, &target, &options).await?;
        print_row(&result);
        if result.interrupted {
            eprintln!("interrupted");
            break;
        }
    }
    drop(server);

    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let options = match parse_args(Args::from_env()) {
        Ok(Command::Run(options)) => *options,
        Ok(Command::Help) => {
            println!("{}", usage());
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("error: {message}\n\n{}", usage());
            return ExitCode::from(2);
        }
    };
    // The read task of the server logs the EOF of a closing connection at error
    // level; the benchmark opens and closes connections in every phase, so that
    // message is silenced unless RUST_LOG asks for it.
    cli::init_tracing("warn,nfs_mamont::task::connection::read=off");

    match run(options).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}
