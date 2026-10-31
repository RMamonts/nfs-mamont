//! Allocator scalability on plain OS threads (no tokio scheduler in the picture):
//! every thread polls `allocate` with a tiny `block_on`.
//!
//! - `loop`     : N threads, allocate + drop, `ring` other buffers kept alive per thread;
//! - `pipeline` : N producers allocate and pass buffers through a channel to N
//!   consumers that drop them (cross-thread free, like parser -> writer).
//!
//! cargo run --release --bin threads

use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::pin;
use std::sync::{mpsc, Arc, Barrier};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::Instant;

use allocator_bench::harness::arg;
use allocator_bench::{buddy, buddy_cached, fixed, lockfree, upstream};
use nfs_mamont::Allocator;

const POOL: usize = 256 << 20;

struct ThreadWaker(thread::Thread);
impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(waker: &Waker, f: F) -> F::Output {
    let mut f = pin!(f);
    let mut cx = Context::from_waker(waker);
    loop {
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => thread::park(),
        }
    }
}

fn nz(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}

fn run_loop<A>(a: Arc<A>, threads: usize, ops: usize, size: usize, ring: usize) -> f64
where
    A: Allocator + Send + Sync + 'static,
    A::Buffer: 'static,
{
    let barrier = Arc::new(Barrier::new(threads + 1));
    let hs: Vec<_> = (0..threads)
        .map(|_| {
            let (a, barrier) = (Arc::clone(&a), Arc::clone(&barrier));
            thread::spawn(move || {
                let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
                let mut held = std::collections::VecDeque::new();
                for _ in 0..ring {
                    held.push_back(block_on(&waker, a.allocate(nz(size))).unwrap());
                }
                barrier.wait();
                for _ in 0..ops {
                    let b = block_on(&waker, a.allocate(nz(size))).unwrap();
                    std::hint::black_box(&b);
                    if ring > 0 {
                        held.push_back(b);
                        drop(held.pop_front());
                    }
                }
            })
        })
        .collect();
    barrier.wait();
    let t = Instant::now();
    for h in hs {
        h.join().unwrap();
    }
    (threads * ops) as f64 / t.elapsed().as_secs_f64() / 1e6
}

fn run_pipeline<A>(a: Arc<A>, pairs: usize, ops: usize, size: usize) -> f64
where
    A: Allocator + Send + Sync + 'static,
    A::Buffer: Send + 'static,
{
    let barrier = Arc::new(Barrier::new(2 * pairs + 1));
    let mut hs = Vec::new();
    for _ in 0..pairs {
        let (tx, rx) = mpsc::sync_channel::<A::Buffer>(256);
        let (a, b1, b2) = (Arc::clone(&a), Arc::clone(&barrier), Arc::clone(&barrier));
        hs.push(thread::spawn(move || {
            let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
            b1.wait();
            for _ in 0..ops {
                tx.send(block_on(&waker, a.allocate(nz(size))).unwrap()).unwrap();
            }
        }));
        hs.push(thread::spawn(move || {
            b2.wait();
            while let Ok(b) = rx.recv() {
                drop(b);
            }
        }));
    }
    barrier.wait();
    let t = Instant::now();
    for h in hs {
        h.join().unwrap();
    }
    (pairs * ops) as f64 / t.elapsed().as_secs_f64() / 1e6
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let reps: usize = arg("reps", 5);
    let ops: usize = arg("ops", 400_000);
    let ring: usize = arg("ring", 8);
    let only: String = arg("only", String::new());
    for &(size, label) in &[(65536usize, "64 KiB"), (4096, "4 KiB")] {
        println!("\n# loop (ring={ring}): Mops/s total, request {label}, median of {reps}");
        println!("{:<26} {:>8} {:>8} {:>8}", "allocator \\ threads", 1, 2, 4);
        macro_rules! lp {
            ($name:expr, $mk:expr) => {
                if only.is_empty() || $name.contains(only.as_str()) {
                    let cells: Vec<f64> = [1usize, 2, 4]
                        .iter()
                        .map(|&t| {
                            median(
                                (0..reps)
                                    .map(|_| run_loop(Arc::new($mk), t, ops / t, size, ring))
                                    .collect(),
                            )
                        })
                        .collect();
                    println!("{:<26} {:>8.2} {:>8.2} {:>8.2}", $name, cells[0], cells[1], cells[2]);
                }
            };
        }
        lp!("upstream Impl(64K)", upstream(65536, POOL / 65536));
        lp!("upstream Impl(4K)", upstream(4096, POOL / 4096));
        lp!("fixed-lifo(64K)", fixed(65536, POOL / 65536, false));
        lp!("fixed-lifo(4K)", fixed(4096, POOL / 4096, false));
        lp!("lockfree(64K)", lockfree(65536, POOL / 65536, false));
        lp!("lockfree(4K)", lockfree(4096, POOL / 4096, false));
        lp!("buddy", buddy(POOL, false));
        lp!("buddy+cache", buddy_cached(POOL, false));

        println!(
            "\n# pipeline (alloc on producer thread, drop on consumer): Mops/s, request {label}"
        );
        println!("{:<26} {:>8} {:>8}", "allocator \\ pairs", 1, 2);
        macro_rules! pp {
            ($name:expr, $mk:expr) => {
                if only.is_empty() || $name.contains(only.as_str()) {
                    let cells: Vec<f64> = [1usize, 2]
                        .iter()
                        .map(|&p| {
                            median(
                                (0..reps)
                                    .map(|_| run_pipeline(Arc::new($mk), p, ops / p, size))
                                    .collect(),
                            )
                        })
                        .collect();
                    println!("{:<26} {:>8.2} {:>8.2}", $name, cells[0], cells[1]);
                }
            };
        }
        pp!("upstream Impl(64K)", upstream(65536, POOL / 65536));
        pp!("upstream Impl(4K)", upstream(4096, POOL / 4096));
        pp!("fixed-lifo(64K)", fixed(65536, POOL / 65536, false));
        pp!("fixed-lifo(4K)", fixed(4096, POOL / 4096, false));
        pp!("lockfree(64K)", lockfree(65536, POOL / 65536, false));
        pp!("lockfree(4K)", lockfree(4096, POOL / 4096, false));
        pp!("buddy", buddy(POOL, false));
        pp!("buddy+cache", buddy_cached(POOL, false));
    }
}
