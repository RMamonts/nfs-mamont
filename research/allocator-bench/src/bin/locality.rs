//! Reuse order vs. cache/TLB locality and RSS.
//!
//! Each operation allocates a buffer, fills it (memcpy in: socket -> buffer or page
//! cache -> buffer), reads it back (memcpy out: buffer -> socket / page cache) and
//! keeps it alive for `ring` further operations (in-flight requests).
//! Upstream recycles buffers FIFO (`ArrayQueue`), so it walks the *whole* pool;
//! the prototypes recycle LIFO, so the working set is ~`ring` buffers.
//!
//! cargo run --release --bin locality -- --alloc upstream --block 65536 --count 2048 --size 65536 --ring 32

use std::collections::VecDeque;
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::time::Instant;

use allocator_bench::harness::arg;
use allocator_bench::rss::{anon_huge, mib, rss};
use allocator_bench::{buddy_cached, fixed, fixed_fifo, fixed_misaligned, lockfree, upstream};
use nfs_mamont::{Allocator, Buffer};

fn run<A: Allocator>(a: A, size: usize, ring: usize, pass_ops: usize, label: &str) {
    let src: Vec<u8> = (0..size).map(|i| i as u8).collect();
    let mut sink = vec![0u8; size];
    let size_nz = NonZeroUsize::new(size).unwrap();
    // No tokio runtime here on purpose: inside one, tokio's coop budget would make the
    // upstream semaphore return Pending every 128 acquisitions.
    {
        let mut held: VecDeque<A::Buffer> = VecDeque::new();
        let mut chunks = 0usize;
        let mut one_pass = |held: &mut VecDeque<A::Buffer>, ops: usize| {
            let t = Instant::now();
            for _ in 0..ops {
                // allocate() never waits here: ring < pool size.
                let mut b = poll_ready(a.allocate(size_nz)).unwrap();
                let mut off = 0;
                for c in b.chunks_mut() {
                    c.copy_from_slice(&src[off..off + c.len()]);
                    off += c.len();
                }
                off = 0;
                for c in b.chunks() {
                    sink[off..off + c.len()].copy_from_slice(c);
                    off += c.len();
                }
                chunks = b.chunks().count();
                black_box(&sink);
                held.push_back(b);
                if held.len() > ring {
                    drop(held.pop_front());
                }
            }
            let el = t.elapsed().as_secs_f64();
            (2 * size * ops) as f64 / el / 1e9
        };
        let rss0 = rss();
        let cold = one_pass(&mut held, pass_ops);
        let rss1 = rss();
        let mut warm = Vec::new();
        for _ in 0..3 {
            warm.push(one_pass(&mut held, pass_ops));
        }
        warm.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{label:<44} ring={ring:>3} chunks={chunks:>2} cold={cold:>6.2} GB/s warm={:>6.2} GB/s  rss_growth={:>7.1} MiB thp={:>6.1} MiB",
            warm[1],
            mib(rss1.saturating_sub(rss0)),
            mib(anon_huge()),
        );
    }
}

/// The allocators never wait in this benchmark, so a no-op waker is enough.
fn poll_ready<F: std::future::Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

fn main() {
    let alloc: String = arg("alloc", "upstream".to_string());
    let block: usize = arg("block", 65536);
    let count: usize = arg("count", 2048);
    let size: usize = arg("size", 65536);
    let ring: usize = arg("ring", 32);
    let huge: bool = arg("huge", false);
    // One "pass" = as many operations as the pool has blocks: FIFO touches every block.
    let pass_ops: usize = arg("ops", count);
    let pool_mib = (block * count) >> 20;
    match alloc.as_str() {
        "upstream" => run(
            upstream(block, count),
            size,
            ring,
            pass_ops,
            &format!("upstream Impl({}K x {count}) = {pool_mib} MiB", block >> 10),
        ),
        "lockfree" => run(
            lockfree(block, count, huge),
            size,
            ring,
            pass_ops,
            &format!(
                "lockfree LIFO({}K x {count}){}",
                block >> 10,
                if huge { " +THP" } else { "" }
            ),
        ),
        "fifo" => run(
            fixed_fifo(block, count, arg("misalign", 0usize), huge),
            size,
            ring,
            pass_ops,
            &format!(
                "control FIFO({}K x {count}) +{}B{}",
                block >> 10,
                arg("misalign", 0usize),
                if huge { " +THP" } else { "" }
            ),
        ),
        "lifo" => run(
            fixed_misaligned(block, count, arg("misalign", 0usize)),
            size,
            ring,
            pass_ops,
            &format!("control LIFO({}K x {count}) +{}B", block >> 10, arg("misalign", 0usize)),
        ),
        "fixed" => run(
            fixed(block, count, huge),
            size,
            ring,
            pass_ops,
            &format!("fixed-lifo({}K x {count})", block >> 10),
        ),
        "buddy" => run(
            buddy_cached(block * count, huge),
            size,
            ring,
            pass_ops,
            &format!("buddy+cache({pool_mib} MiB){}", if huge { " +THP" } else { "" }),
        ),
        other => panic!("unknown allocator {other}"),
    }
}
