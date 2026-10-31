//! End-to-end: the real nfs-mamont server (`handle_forever`) with a mock VFS, driven
//! over loopback TCP by a pipelining NFSv3 READ/WRITE client. Only the allocator
//! type changes between runs.
//!
//! cargo run --release --bin e2e -- --alloc upstream64k --op read --size 65536 --mode mem
//!
//! Backend modes (how the mock VFS moves data in/out of the allocator buffer):
//! - `mem`      : memcpy from/to RAM (CPU-only backend);
//! - `pread`    : one pread/pwrite syscall per buffer chunk (tmpfs file);
//! - `preadv`   : one preadv/pwritev syscall for all chunks;
//! - `blocking` : one `spawn_blocking` pread/pwrite per chunk --- the cost model of
//!   `tokio::fs`, which the mirror-fs demo uses chunk by chunk.

use std::cell::RefCell;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use allocator_bench::harness::arg;
use allocator_bench::{buddy_cached, fixed, fixed_fifo, fixed_misaligned, lockfree, upstream};
use nfs_mamont::auth::Credential;
use nfs_mamont::vfs::{self, file, read, write};
use nfs_mamont::{handle_forever, service, Allocator, Buffer, ServerContext};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Mem,
    Pread,
    Preadv,
    Blocking,
}

const FILE_SIZE: u64 = 64 << 20;

struct MockFs {
    mode: Mode,
    src: Vec<u8>,
    file: File,
}

thread_local! {
    static SINK: RefCell<Vec<u8>> = RefCell::new(vec![0u8; 1 << 20]);
}

#[derive(Clone, Copy)]
struct SendPtr(usize, usize);
unsafe impl Send for SendPtr {}

impl MockFs {
    async fn blocking_io(&self, ptr: SendPtr, off: u64, write: bool) {
        let fd = self.file.as_raw_fd();
        tokio::task::spawn_blocking(move || unsafe {
            if write {
                libc::pwrite(fd, ptr.0 as *const _, ptr.1, off as i64);
            } else {
                libc::pread(fd, ptr.0 as *mut _, ptr.1, off as i64);
            }
        })
        .await
        .unwrap();
    }
}

impl<B: Buffer> read::Read<B> for MockFs {
    async fn read(
        &self,
        args: read::Args,
        mut data: B,
        _cred: &Credential,
    ) -> Result<read::Success<B>, read::Fail> {
        let count = (args.count as usize).min(data.len());
        let base = args.offset % (FILE_SIZE - (1 << 20));
        match self.mode {
            Mode::Mem => {
                let mut off = 0;
                for c in data.chunks_mut() {
                    let n = c.len().min(count - off);
                    c[..n].copy_from_slice(&self.src[off..off + n]);
                    off += n;
                }
            }
            Mode::Pread => {
                let mut off = 0;
                for c in data.chunks_mut() {
                    self.file.read_exact_at(c, base + off).unwrap();
                    off += c.len() as u64;
                }
            }
            Mode::Preadv => {
                let iov: Vec<libc::iovec> = data
                    .chunks_mut()
                    .map(|c| libc::iovec { iov_base: c.as_mut_ptr() as *mut _, iov_len: c.len() })
                    .collect();
                unsafe {
                    libc::preadv(self.file.as_raw_fd(), iov.as_ptr(), iov.len() as i32, base as i64)
                };
            }
            Mode::Blocking => {
                let mut off = 0;
                for c in data.chunks_mut() {
                    let p = SendPtr(c.as_mut_ptr() as usize, c.len());
                    self.blocking_io(p, base + off, false).await;
                    off += c.len() as u64;
                }
            }
        }
        Ok(read::Success {
            head: read::SuccessHeader { file_attr: None, count: count as u32, eof: false },
            data,
        })
    }
}

impl<B: Buffer> write::Write<B> for MockFs {
    async fn write(
        &self,
        args: write::Args<B>,
        _cred: &Credential,
    ) -> Result<write::Success, write::Fail> {
        let base = args.offset % (FILE_SIZE - (1 << 20));
        match self.mode {
            Mode::Mem => SINK.with(|s| {
                let mut s = s.borrow_mut();
                let mut off = 0;
                for c in args.data.chunks() {
                    s[off..off + c.len()].copy_from_slice(c);
                    off += c.len();
                }
            }),
            Mode::Pread => {
                let mut off = 0;
                for c in args.data.chunks() {
                    self.file.write_all_at(c, base + off).unwrap();
                    off += c.len() as u64;
                }
            }
            Mode::Preadv => {
                let iov: Vec<libc::iovec> = args
                    .data
                    .chunks()
                    .map(|c| libc::iovec { iov_base: c.as_ptr() as *mut _, iov_len: c.len() })
                    .collect();
                unsafe {
                    libc::pwritev(
                        self.file.as_raw_fd(),
                        iov.as_ptr(),
                        iov.len() as i32,
                        base as i64,
                    )
                };
            }
            Mode::Blocking => {
                let mut off = 0;
                let ptrs: Vec<SendPtr> =
                    args.data.chunks().map(|c| SendPtr(c.as_ptr() as usize, c.len())).collect();
                for p in ptrs {
                    self.blocking_io(p, base + off, true).await;
                    off += p.1 as u64;
                }
            }
        }
        Ok(write::Success {
            file_wcc: vfs::WccData { before: None, after: None },
            count: args.size,
            committed: args.stable,
            verifier: write::Verifier([0; 8]),
        })
    }
}

macro_rules! unsupported {
    ($($module:ident :: $tr:ident :: $method:ident { $($field:ident : $val:expr),* }),* $(,)?) => {$(
        impl vfs::$module::$tr for MockFs {
            async fn $method(
                &self,
                _args: vfs::$module::Args,
                _cred: &Credential,
            ) -> Result<vfs::$module::Success, vfs::$module::Fail> {
                Err(vfs::$module::Fail { error: vfs::Error::NotSupported $(, $field: $val)* })
            }
        }
    )*};
}

fn wcc() -> vfs::WccData {
    vfs::WccData { before: None, after: None }
}

unsupported! {
    get_attr::GetAttr::get_attr {},
    set_attr::SetAttr::set_attr { wcc_data: wcc() },
    lookup::Lookup::lookup { dir_attr: None },
    access::Access::access { object_attr: None },
    read_link::ReadLink::read_link { symlink_attr: None },
    create::Create::create { wcc_data: wcc() },
    mk_dir::MkDir::mk_dir { dir_wcc: wcc() },
    symlink::Symlink::symlink { dir_wcc: wcc() },
    mk_node::MkNode::mk_node { dir_wcc: wcc() },
    remove::Remove::remove { dir_wcc: wcc() },
    rm_dir::RmDir::rm_dir { dir_wcc: wcc() },
    rename::Rename::rename { from_dir_wcc: wcc(), to_dir_wcc: wcc() },
    link::Link::link { file_attr: None, dir_wcc: wcc() },
    read_dir::ReadDir::read_dir { dir_attr: None },
    read_dir_plus::ReadDirPlus::read_dir_plus { dir_attr: None },
    fs_stat::FsStat::fs_stat { root_attr: None },
    fs_info::FsInfo::fs_info { root_attr: None },
    path_conf::PathConf::path_conf { file_attr: None },
    commit::Commit::commit { file_wcc: wcc() },
}

// ---------------------------------------------------------------------------------
// Client side: hand-encoded ONC RPC calls.

fn put(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn call(proc_: u32, fh: &file::Handle, offset: u64, size: usize) -> Vec<u8> {
    let mut b = vec![0u8; 4]; // record mark, patched below
    for x in [1u32, 0, 2, 100_003, 3, proc_, 0, 0, 0, 0] {
        put(&mut b, x); // xid, CALL, rpcvers, prog, vers, proc, cred NONE, verf NONE
    }
    put(&mut b, fh.0.len() as u32);
    b.extend_from_slice(&fh.0);
    b.resize(b.len() + (4 - fh.0.len() % 4) % 4, 0);
    b.extend_from_slice(&offset.to_be_bytes());
    put(&mut b, size as u32);
    if proc_ == 7 {
        put(&mut b, 2); // FILE_SYNC
        put(&mut b, size as u32);
        b.extend((0..size).map(|i| i as u8));
        b.resize(b.len() + (4 - size % 4) % 4, 0);
    }
    let len = (b.len() - 4) as u32 | 0x8000_0000;
    b[..4].copy_from_slice(&len.to_be_bytes());
    b
}

static DONE: AtomicU64 = AtomicU64::new(0);
static BAD: AtomicU64 = AtomicU64::new(0);

async fn connection(port: u16, req: Vec<u8>, depth: usize) {
    let s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s.set_nodelay(true).unwrap();
    let (mut rd, mut wr) = s.into_split();
    let sem = Arc::new(Semaphore::new(depth));
    let sem2 = Arc::clone(&sem);
    tokio::spawn(async move {
        let mut req = req;
        let mut xid = 1u32;
        loop {
            sem2.acquire().await.unwrap().forget();
            xid = xid.wrapping_add(1);
            req[4..8].copy_from_slice(&xid.to_be_bytes());
            if wr.write_all(&req).await.is_err() {
                return;
            }
        }
    });
    let mut body = vec![0u8; 2 << 20];
    loop {
        let mut mark = [0u8; 4];
        if rd.read_exact(&mut mark).await.is_err() {
            return;
        }
        let len = (u32::from_be_bytes(mark) & 0x7fff_ffff) as usize;
        rd.read_exact(&mut body[..len]).await.unwrap();
        // xid | REPLY | MSG_ACCEPTED | verf(flavor,len) | accept_stat | nfsstat3
        let word = |i: usize| u32::from_be_bytes(body[i * 4..i * 4 + 4].try_into().unwrap());
        if len < 28 || word(1) != 1 || word(2) != 0 || word(5) != 0 || word(6) != 0 {
            BAD.fetch_add(1, Relaxed);
        }
        DONE.fetch_add(1, Relaxed);
        sem.add_permits(1);
    }
}

// ---------------------------------------------------------------------------------
// Server CPU accounting: on-CPU nanoseconds of the server runtime's threads.

static SERVER_TIDS: Mutex<Vec<i32>> = Mutex::new(Vec::new());

fn server_cpu_ns() -> u64 {
    SERVER_TIDS
        .lock()
        .unwrap()
        .iter()
        .filter_map(|tid| std::fs::read_to_string(format!("/proc/self/task/{tid}/schedstat")).ok())
        .filter_map(|s| s.split_whitespace().next().and_then(|x| x.parse::<u64>().ok()))
        .sum()
}

async fn serve<A>(alloc: A, mode: Mode, listener: TcpListener)
where
    A: Allocator + Send + Sync + 'static,
    A::Buffer: 'static,
{
    let path = format!("/dev/shm/nfs-mamont-bench-{}", std::process::id());
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .unwrap();
    file.set_len(FILE_SIZE).unwrap();
    file.write_all_at(&vec![7u8; FILE_SIZE as usize], 0).unwrap();
    let _ = std::fs::remove_file(&path);
    let fs = Arc::new(MockFs { mode, src: (0..2 << 20).map(|i| i as u8).collect(), file });
    let ctx = ServerContext::new(Arc::new(alloc), std::num::NonZeroUsize::new(16).unwrap());
    ctx.add_backend(fs).unwrap();
    let mount = Arc::new(service::mount::MountService::with_exports(Vec::new()));
    let nlm = Arc::new(service::nlm::NlmService::new());
    handle_forever(listener, ctx, mount, nlm).await.unwrap();
}

fn main() {
    let alloc: String = arg("alloc", "upstream64k".to_string());
    let op: String = arg("op", "read".to_string());
    let size: usize = arg("size", 65536);
    let mode = match arg("mode", "mem".to_string()).as_str() {
        "mem" => Mode::Mem,
        "pread" => Mode::Pread,
        "preadv" => Mode::Preadv,
        "blocking" => Mode::Blocking,
        m => panic!("unknown mode {m}"),
    };
    let conns: usize = arg("conns", 2);
    let depth: usize = arg("depth", 16);
    let secs: f64 = arg("secs", 3.0);
    let workers: usize = arg("workers", 3);

    let server_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .on_thread_start(|| SERVER_TIDS.lock().unwrap().push(unsafe { libc::gettid() }))
        .enable_all()
        .build()
        .unwrap();
    let listener = server_rt.block_on(TcpListener::bind("127.0.0.1:0")).unwrap();
    let port = listener.local_addr().unwrap().port();
    let a = alloc.clone();
    server_rt.spawn(async move {
        match a.as_str() {
            "upstream64k" => serve(upstream(65536, 2048), mode, listener).await,
            "upstream1m" => serve(upstream(1 << 20, 2048), mode, listener).await,
            "upstream4k" => serve(upstream(4096, 32768), mode, listener).await,
            "fixed64k" => serve(fixed(65536, 2048, false), mode, listener).await,
            // 2x2 control: reuse order {fifo, lifo} x base alignment {page, +16 bytes}
            "fifo64k" => serve(fixed_fifo(65536, 2048, 0, false), mode, listener).await,
            "fifo64k+16" => serve(fixed_fifo(65536, 2048, 16, false), mode, listener).await,
            "lifo64k+16" => serve(fixed_misaligned(65536, 2048, 16), mode, listener).await,
            "lockfree64k" => serve(lockfree(65536, 2048, false), mode, listener).await,
            "lockfree4k" => serve(lockfree(4096, 32768, false), mode, listener).await,
            "buddy" => serve(buddy_cached(128 << 20, false), mode, listener).await,
            other => panic!("unknown allocator {other}"),
        }
    });

    let fh = file::Handle::new(0, [0u8; file::PAYLOAD_SIZE]);
    let proc_ = if op == "write" { 7 } else { 6 };
    let client_rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    client_rt.block_on(async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        for i in 0..conns {
            let req = call(proc_, &fh, (i as u64) << 22, size);
            tokio::spawn(connection(port, req, depth));
        }
        tokio::time::sleep(Duration::from_millis(700)).await; // warm-up
        let (d0, c0, t0) = (DONE.load(Relaxed), server_cpu_ns(), Instant::now());
        tokio::time::sleep(Duration::from_secs_f64(secs)).await;
        let (d1, c1, el) = (DONE.load(Relaxed), server_cpu_ns(), t0.elapsed().as_secs_f64());
        let ops = (d1 - d0) as f64;
        println!(
            "alloc={alloc:<12} op={op:<5} size={size:>7} mode={:<8} ops/s={:>9.0} MB/s={:>8.1} server_cpu_us/op={:>6.2} server_cpu%={:>5.0} bad={}",
            arg("mode", "mem".to_string()),
            ops / el,
            ops * size as f64 / el / 1e6,
            (c1 - c0) as f64 / 1e3 / ops,
            (c1 - c0) as f64 / 1e9 / el * 100.0,
            BAD.load(Relaxed),
        );
    });
    std::process::exit(0);
}
