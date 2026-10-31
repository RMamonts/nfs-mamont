//! Microbenchmarks of the per-connection in-flight limiter.
//!
//! * `cycle`    — acquire + release in one task, never parks: raw hot-path cost.
//! * `pipeline` — read task -> shared pool channel -> P workers -> per-connection
//!                result channel -> write task, like the server's request path. The
//!                token travels with the item and is released by the write task.
//! * `stress`   — limit 1/2 with read and write on different threads, watchdog for
//!                lost wake-ups.

mod alt;
mod budget;

use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use alt::{BatchBudget, Branch, Limit, NoLimit, RaiiBudget, SemForget, SemOwned};

const REPEATS: usize = 7;

fn median(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[0], v[v.len() / 2], v[v.len() - 1])
}

// ---------------------------------------------------------------- cycle

async fn cycle_once<L: Limit>(n: u64, limit: usize) -> f64 {
    let (mut l, rel) = L::create(limit);
    let start = Instant::now();
    for _ in 0..n {
        let t = l.acquire().await;
        L::release(&rel, black_box(t));
    }
    start.elapsed().as_nanos() as f64 / n as f64
}

fn cycle<L: Limit>(n: u64) {
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let mut res = Vec::new();
    for _ in 0..REPEATS {
        res.push(rt.block_on(cycle_once::<L>(n, 256)));
    }
    let (min, med, max) = median(res);
    println!("cycle    {:<24} {:>7.2} ns/op (min {:.2}, max {:.2})", L::NAME, med, min, max);
}

// ---------------------------------------------------------------- pipeline

struct Request<T> {
    token: T,
    reply_tx: async_channel::Sender<Reply<T>>,
    _payload: [u64; 8],
}
struct Reply<T> {
    token: T,
    _payload: [u64; 8],
}

async fn pipeline_once<L: Limit>(n: u64, limit: usize, workers: usize) -> f64 {
    let (mut l, rel) = L::create(limit);
    let (pool_tx, pool_rx) = async_channel::unbounded::<Request<L::Token>>();
    let (result_tx, result_rx) = async_channel::unbounded::<Reply<L::Token>>();

    let mut pool = Vec::new();
    for _ in 0..workers {
        let rx = pool_rx.clone();
        pool.push(tokio::spawn(async move {
            while let Ok(Request { token, reply_tx, _payload }) = rx.recv().await {
                let _ = reply_tx.send(Reply { token, _payload }).await;
            }
        }));
    }
    drop(pool_rx);

    let start = Instant::now();
    let writer = tokio::spawn(async move {
        let mut done = 0u64;
        while let Ok(Reply { token, _payload }) = result_rx.recv().await {
            L::release(&rel, token);
            done += 1;
            if done == n {
                break;
            }
        }
        done
    });
    let reader = tokio::spawn(async move {
        for i in 0..n {
            let token = l.acquire().await;
            let req = Request { token, reply_tx: result_tx.clone(), _payload: [i; 8] };
            pool_tx.send(req).await.unwrap();
        }
    });
    reader.await.unwrap();
    let done = writer.await.unwrap();
    assert_eq!(done, n);
    let elapsed = start.elapsed();
    for p in pool {
        p.abort();
    }
    elapsed.as_nanos() as f64 / n as f64
}

fn pipeline<L: Limit>(n: u64, limit: usize, threads: usize, workers: usize) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(threads)
        .build()
        .unwrap();
    let mut res = Vec::new();
    for _ in 0..REPEATS {
        res.push(rt.block_on(pipeline_once::<L>(n, limit, workers)));
    }
    let (min, med, max) = median(res);
    println!(
        "pipeline thr={threads} limit={:<8} {:<24} {:>7.1} ns/item (min {:.1}, max {:.1})",
        limit,
        L::NAME,
        med,
        min,
        max
    );
}

// ---------------------------------------------------------------- stress

fn stress<L: Limit>(n: u64, limit: usize) {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_time().build().unwrap();
    let progress = Arc::new(AtomicU64::new(0));
    let p2 = Arc::clone(&progress);
    let start = Instant::now();
    let ok = rt.block_on(async move {
        let (mut l, rel) = L::create(limit);
        let (tx, rx) = async_channel::unbounded::<L::Token>();
        let writer = tokio::spawn(async move {
            while let Ok(t) = rx.recv().await {
                // yield sometimes so read and write interleave on different workers
                if p2.fetch_add(1, Ordering::Relaxed) % 7 == 0 {
                    tokio::task::yield_now().await;
                }
                L::release(&rel, t);
            }
        });
        let reader = tokio::spawn(async move {
            for _ in 0..n {
                let t = l.acquire().await;
                tx.send(t).await.unwrap();
            }
        });
        let mut last = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let now = progress.load(Ordering::Relaxed);
            if reader.is_finished() {
                writer.abort();
                return true;
            }
            if now == last {
                return false; // no progress for 2 s: lost wake-up
            }
            last = now;
        }
    });
    println!(
        "stress   {:<24} limit={limit} n={n}: {} in {:.2?}",
        L::NAME,
        if ok { "OK" } else { "HANG (lost wake-up)" },
        start.elapsed()
    );
}

fn main() {
    let what = std::env::args().nth(1).unwrap_or_else(|| "all".into());
    if what == "all" || what == "cycle" {
        let n = 20_000_000;
        cycle::<NoLimit>(n);
        cycle::<Branch>(n);
        cycle::<RaiiBudget>(n);
        cycle::<BatchBudget>(n);
        cycle::<SemForget>(n);
        cycle::<SemOwned>(n);
    }
    if what == "all" || what == "pipeline" {
        let n = 2_000_000;
        let threads: usize =
            std::env::var("THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
        let workers: usize =
            std::env::var("WORKERS").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
        for limit in [16usize, 256, 1 << 22] {
            pipeline::<NoLimit>(n, limit, threads, workers);
            pipeline::<Branch>(n, limit, threads, workers);
            pipeline::<RaiiBudget>(n, limit, threads, workers);
            pipeline::<BatchBudget>(n, limit, threads, workers);
            pipeline::<SemForget>(n, limit, threads, workers);
            pipeline::<SemOwned>(n, limit, threads, workers);
        }
    }
    if what == "all" || what == "stress" {
        for limit in [1usize, 2, 3] {
            stress::<Branch>(20_000_000, limit);
            stress::<BatchBudget>(20_000_000, limit);
        }
    }
}
