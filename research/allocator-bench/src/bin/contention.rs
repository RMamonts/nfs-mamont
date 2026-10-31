//! Multi-threaded throughput of allocate + drop.
//!
//! Scenarios:
//! - `loop`: T tasks, each allocates and drops in a loop (yield every 8 ops);
//! - `pipeline`: T producers allocate and send buffers through a channel to T
//!   consumers that drop them (alloc and free on different threads, like the
//!   parser -> VFS/writer hand-off in nfs-mamont);
//! - `starved`: pool much smaller than demand (64 x 64 KiB) and 256 tasks, exercising
//!   the waiting / wake-up path.
//!
//! cargo run --release --bin contention

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Instant;

use allocator_bench::harness::{arg, multi_thread, percentile};
use allocator_bench::{buddy, fixed, upstream};
use nfs_mamont::Allocator;
use tokio::sync::Barrier;

const POOL: usize = 256 << 20;

fn nz(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}

async fn loop_scenario<A>(a: Arc<A>, tasks: usize, ops: usize, size: usize) -> f64
where
    A: Allocator + Send + Sync + 'static,
    A::Buffer: 'static,
{
    let barrier = Arc::new(Barrier::new(tasks + 1));
    let mut hs = Vec::new();
    for _ in 0..tasks {
        let (a, barrier) = (Arc::clone(&a), Arc::clone(&barrier));
        hs.push(tokio::spawn(async move {
            barrier.wait().await;
            for i in 0..ops {
                let b = a.allocate(nz(size)).await.unwrap();
                std::hint::black_box(&b);
                drop(b);
                if i % 8 == 7 {
                    tokio::task::yield_now().await;
                }
            }
        }));
    }
    barrier.wait().await;
    let t = Instant::now();
    for h in hs {
        h.await.unwrap();
    }
    (tasks * ops) as f64 / t.elapsed().as_secs_f64() / 1e6
}

async fn pipeline_scenario<A>(a: Arc<A>, pairs: usize, ops: usize, size: usize) -> f64
where
    A: Allocator + Send + Sync + 'static,
    A::Buffer: 'static,
{
    let barrier = Arc::new(Barrier::new(2 * pairs + 1));
    let mut hs = Vec::new();
    for _ in 0..pairs {
        let (tx, rx) = async_channel::bounded::<A::Buffer>(64);
        let (a, b1, b2) = (Arc::clone(&a), Arc::clone(&barrier), Arc::clone(&barrier));
        hs.push(tokio::spawn(async move {
            b1.wait().await;
            for _ in 0..ops {
                let b = a.allocate(nz(size)).await.unwrap();
                tx.send(b).await.ok().unwrap();
            }
        }));
        hs.push(tokio::spawn(async move {
            b2.wait().await;
            while let Ok(b) = rx.recv().await {
                drop(b);
            }
        }));
    }
    barrier.wait().await;
    let t = Instant::now();
    for h in hs {
        h.await.unwrap();
    }
    (pairs * ops) as f64 / t.elapsed().as_secs_f64() / 1e6
}

/// Returns (Mops/s, p50 us, p99 us) of allocate latency.
async fn starved_scenario<A>(a: Arc<A>, tasks: usize, ops: usize, size: usize) -> (f64, f64, f64)
where
    A: Allocator + Send + Sync + 'static,
    A::Buffer: 'static,
{
    let barrier = Arc::new(Barrier::new(tasks + 1));
    let mut hs = Vec::new();
    for _ in 0..tasks {
        let (a, barrier) = (Arc::clone(&a), Arc::clone(&barrier));
        hs.push(tokio::spawn(async move {
            barrier.wait().await;
            let mut lat = Vec::with_capacity(ops);
            for _ in 0..ops {
                let t = Instant::now();
                let b = a.allocate(nz(size)).await.unwrap();
                lat.push(t.elapsed().as_nanos() as f64 / 1e3);
                tokio::task::yield_now().await;
                drop(b);
            }
            lat
        }));
    }
    barrier.wait().await;
    let t = Instant::now();
    let mut all = Vec::new();
    for h in hs {
        all.extend(h.await.unwrap());
    }
    let mops = (tasks * ops) as f64 / t.elapsed().as_secs_f64() / 1e6;
    let p50 = percentile(&mut all, 0.5);
    let p99 = percentile(&mut all, 0.99);
    (mops, p50, p99)
}

fn row(name: &str, cells: &[String]) {
    print!("{name:<30}");
    for c in cells {
        print!(" {c:>10}");
    }
    println!();
}

fn main() {
    let workers: usize = arg("workers", 4);
    let reps: usize = arg("reps", 3);
    let rt = multi_thread(workers);
    let median = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };

    for &(size, label) in &[(65536usize, "64 KiB"), (4096, "4 KiB")] {
        println!("\n# loop: Mops/s, request {label}, {workers} workers (median of {reps})");
        let ts = [1usize, 4, 16, 64];
        row("allocator \\ tasks", &ts.iter().map(|t| t.to_string()).collect::<Vec<_>>());
        macro_rules! run_loop {
            ($name:expr, $mk:expr) => {{
                let cells: Vec<String> = ts
                    .iter()
                    .map(|&t| {
                        let v: Vec<f64> = (0..reps)
                            .map(|_| {
                                rt.block_on(loop_scenario(Arc::new($mk), t, 200_000 / t, size))
                            })
                            .collect();
                        format!("{:.2}", median(v))
                    })
                    .collect();
                row($name, &cells);
            }};
        }
        run_loop!("upstream Impl(64K)", upstream(65536, POOL / 65536));
        run_loop!("upstream Impl(4K)", upstream(4096, POOL / 4096));
        run_loop!("fixed-lifo(64K)", fixed(65536, POOL / 65536, false));
        run_loop!("fixed-lifo(4K)", fixed(4096, POOL / 4096, false));
        run_loop!("buddy(4K pages)", buddy(POOL, false));

        println!("\n# pipeline (alloc on producer, drop on consumer): Mops/s, request {label}");
        let ps = [1usize, 2, 4];
        row("allocator \\ pairs", &ps.iter().map(|t| t.to_string()).collect::<Vec<_>>());
        macro_rules! run_pipe {
            ($name:expr, $mk:expr) => {{
                let cells: Vec<String> = ps
                    .iter()
                    .map(|&p| {
                        let v: Vec<f64> = (0..reps)
                            .map(|_| {
                                rt.block_on(pipeline_scenario(Arc::new($mk), p, 200_000 / p, size))
                            })
                            .collect();
                        format!("{:.2}", median(v))
                    })
                    .collect();
                row($name, &cells);
            }};
        }
        run_pipe!("upstream Impl(64K)", upstream(65536, POOL / 65536));
        run_pipe!("upstream Impl(4K)", upstream(4096, POOL / 4096));
        run_pipe!("fixed-lifo(64K)", fixed(65536, POOL / 65536, false));
        run_pipe!("fixed-lifo(4K)", fixed(4096, POOL / 4096, false));
        run_pipe!("buddy(4K pages)", buddy(POOL, false));
    }

    println!("\n# starved: pool = 64 x 64 KiB, 256 tasks, request 64 KiB, hold = 1 yield");
    row("allocator", &["Mops/s".into(), "p50 us".into(), "p99 us".into()]);
    macro_rules! run_starved {
        ($name:expr, $mk:expr) => {{
            let mut r: Vec<(f64, f64, f64)> = (0..reps)
                .map(|_| rt.block_on(starved_scenario(Arc::new($mk), 256, 2000, 65536)))
                .collect();
            r.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            let (m, p50, p99) = r[r.len() / 2];
            row($name, &[format!("{m:.2}"), format!("{p50:.1}"), format!("{p99:.1}")]);
        }};
    }
    run_starved!("upstream Impl(64K)", upstream(65536, 64));
    run_starved!("fixed-lifo(64K)", fixed(65536, 64, false));
    run_starved!("buddy(4K pages)", buddy(64 * 65536, false));
}
