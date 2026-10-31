//! Minimal measurement helpers (no external benchmark framework, so the numbers are
//! easy to reproduce with plain `cargo run --release`).

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct Stats {
    /// Nanoseconds per operation.
    pub median: f64,
    pub min: f64,
    pub p90: f64,
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:>9.1} ns/op (min {:>8.1}, p90 {:>8.1})", self.median, self.min, self.p90)
    }
}

/// Runs `samples` batches of `ops` operations; `f(ops)` must perform them and return
/// the elapsed time. One extra warm-up batch is discarded.
pub fn sample(samples: usize, ops: u64, mut f: impl FnMut(u64) -> Duration) -> Stats {
    f(ops);
    let mut v: Vec<f64> = (0..samples).map(|_| f(ops).as_nanos() as f64 / ops as f64).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Stats { median: v[v.len() / 2], min: v[0], p90: v[(v.len() * 9) / 10] }
}

/// Times a closure.
pub fn time(f: impl FnOnce()) -> Duration {
    let t = Instant::now();
    f();
    t.elapsed()
}

pub fn current_thread() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
}

pub fn multi_thread(workers: usize) -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .unwrap()
}

/// Percentile of an unsorted slice (sorts it).
pub fn percentile(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((v.len() as f64 - 1.0) * p).round() as usize;
    v[idx]
}

/// Parses `--key value` style arguments.
pub fn arg<T: std::str::FromStr>(name: &str, default: T) -> T {
    let args: Vec<String> = std::env::args().collect();
    for w in args.windows(2) {
        if w[0] == format!("--{name}") {
            if let Ok(v) = w[1].parse() {
                return v;
            }
        }
    }
    default
}
