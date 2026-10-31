//! Head-of-line blocking: what one oversized request (e.g. a READ whose `count` is not
//! clamped to rtmax) does to everybody else.
//!
//! 32 "normal" tasks allocate 64 KiB and hold it ~100 us. Optionally, one "greedy"
//! task repeatedly asks for `greedy` bytes and holds them for 2 ms. The pool and the
//! semaphore are FIFO, so while the greedy request waits at the head of the queue,
//! every small request waits behind it even if memory is free.
//!
//! cargo run --release --bin hol

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Arc;
use std::time::{Duration, Instant};

use allocator_bench::harness::{multi_thread, percentile};
use allocator_bench::upstream;
use nfs_mamont::Allocator;

const BLOCK: usize = 65536;
const COUNT: usize = 256; // 16 MiB pool

async fn scenario(greedy: Option<usize>) -> (f64, f64, f64) {
    let a = Arc::new(upstream(BLOCK, COUNT));
    let stop = Arc::new(AtomicBool::new(false));
    if let Some(g) = greedy {
        let (a, stop) = (Arc::clone(&a), Arc::clone(&stop));
        tokio::spawn(async move {
            while !stop.load(Relaxed) {
                let b = a.allocate(NonZeroUsize::new(g).unwrap()).await.unwrap();
                tokio::time::sleep(Duration::from_millis(2)).await;
                drop(b);
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
    }
    let mut hs = Vec::new();
    for _ in 0..32 {
        let a = Arc::clone(&a);
        hs.push(tokio::spawn(async move {
            let mut lat = Vec::new();
            let end = Instant::now() + Duration::from_secs(2);
            while Instant::now() < end {
                let t = Instant::now();
                let b = a.allocate(NonZeroUsize::new(BLOCK).unwrap()).await.unwrap();
                lat.push(t.elapsed().as_secs_f64() * 1e6);
                spin(Duration::from_micros(100)).await;
                drop(b);
            }
            lat
        }));
    }
    let mut all = Vec::new();
    for h in hs {
        all.extend(h.await.unwrap());
    }
    stop.store(true, Relaxed);
    let n = all.len() as f64 / 2.0;
    (n, percentile(&mut all, 0.5), percentile(&mut all, 0.99))
}

/// Simulated work that keeps the buffer busy without parking the worker for long.
async fn spin(d: Duration) {
    let end = Instant::now() + d;
    while Instant::now() < end {
        tokio::task::yield_now().await;
    }
}

fn main() {
    let rt = multi_thread(4);
    println!(
        "pool = {COUNT} x 64 KiB = {} MiB; 32 tasks allocate 64 KiB, hold 100 us",
        (COUNT * BLOCK) >> 20
    );
    println!("{:<44} {:>12} {:>10} {:>10}", "scenario", "small/s", "p50 us", "p99 us");
    for (name, g) in [
        ("no greedy request", None),
        ("greedy READ count = 1 MiB (clamped to rtmax)", Some(1 << 20)),
        ("greedy READ count = whole pool (16 MiB)", Some(COUNT * BLOCK)),
    ] {
        let (n, p50, p99) = rt.block_on(scenario(g));
        println!("{name:<44} {n:>12.0} {p50:>10.1} {p99:>10.1}");
    }
}
