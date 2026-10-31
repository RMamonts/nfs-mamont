//! Single-threaded micro benchmarks: cost of `allocate` + drop, cost breakdown of the
//! upstream fast path, and alignment of the handed-out memory.
//!
//! cargo run --release --bin micro

use std::collections::VecDeque;
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use allocator_bench::harness::{current_thread, sample, Stats};
use allocator_bench::{buddy, buddy_cached, fixed, lockfree, upstream, PAGE};
use crossbeam_queue::ArrayQueue;
use nfs_mamont::{Allocator, Buffer, Slice, UnownedBuffer};
use tokio::runtime::Runtime;
use tokio::sync::Semaphore;

const POOL: usize = 128 << 20;
const SAMPLES: usize = 15;

fn nz(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}

/// Chooses a batch size so that one sample takes roughly 20 ms.
fn calibrate(mut f: impl FnMut(u64) -> Duration) -> u64 {
    let probe = 200;
    let t = f(probe).as_nanos().max(1) as u64 / probe;
    (20_000_000 / t.max(1)).clamp(200, 2_000_000)
}

/// `allocate` + drop with `ring` other buffers kept alive (in-flight requests).
fn bench_alloc<A: Allocator>(rt: &Runtime, a: &A, size: usize, ring: usize) -> Stats {
    let size = nz(size);
    let mut held: VecDeque<A::Buffer> = VecDeque::new();
    rt.block_on(async {
        for _ in 0..ring {
            held.push_back(a.allocate(size).await.unwrap());
        }
    });
    let mut run = |ops: u64| {
        rt.block_on(async {
            let t = Instant::now();
            for _ in 0..ops {
                let b = a.allocate(size).await.unwrap();
                black_box(&b);
                if ring == 0 {
                    drop(b);
                } else {
                    held.push_back(b);
                    drop(held.pop_front());
                }
            }
            t.elapsed()
        })
    };
    let ops = calibrate(&mut run);
    sample(SAMPLES, ops, run)
}

fn breakdown(rt: &Runtime) {
    println!("\n## Upstream fast path, cost of the individual steps (n = buffers per request)");
    println!("{:<58} {:>40}", "step", "time");
    let mut backing = vec![0u8; 256 * 64];
    let base = backing.as_mut_ptr();
    for n in [1usize, 16, 256] {
        // tokio semaphore: acquire_many(n).await + forget + add_permits(n)
        let sem = Semaphore::new(4096);
        let s = sample(SAMPLES, 200_000, |ops| {
            rt.block_on(async {
                let t = Instant::now();
                for _ in 0..ops {
                    let p = sem.acquire_many(n as u32).await.unwrap();
                    p.forget();
                    sem.add_permits(n);
                }
                t.elapsed()
            })
        });
        println!("{:<58} {}", format!("tokio Semaphore acquire_many+add_permits (n={n})"), s);

        let sem = Semaphore::new(4096);
        let s = sample(SAMPLES, 200_000, |ops| {
            let t = Instant::now();
            for _ in 0..ops {
                let p = sem.try_acquire_many(n as u32).unwrap();
                p.forget();
                sem.add_permits(n);
            }
            t.elapsed()
        });
        println!("{:<58} {}", format!("  same via try_acquire_many (no future)  (n={n})"), s);

        // ArrayQueue: n pops + n pushes
        let q = ArrayQueue::new(4096);
        for i in 0..4096 {
            q.push(unsafe { UnownedBuffer::from_raw_parts(base.add(i % 256), 1) }).unwrap();
        }
        let mut tmp = Vec::with_capacity(n);
        let s = sample(SAMPLES, (2_000_000 / n as u64).max(1000), |ops| {
            let t = Instant::now();
            for _ in 0..ops {
                for _ in 0..n {
                    tmp.push(q.pop().unwrap());
                }
                for b in tmp.drain(..) {
                    let _ = q.push(b);
                }
            }
            t.elapsed()
        });
        println!("{:<58} {}", format!("ArrayQueue n×pop + n×push (n={n})"), s);

        // Vec<UnownedBuffer>::with_capacity(n) + n pushes + drop
        let s = sample(SAMPLES, (2_000_000 / n as u64).max(1000), |ops| {
            let t = Instant::now();
            for _ in 0..ops {
                let mut v = Vec::with_capacity(n);
                for i in 0..n {
                    v.push(unsafe { UnownedBuffer::from_raw_parts(base.add(i), 64) });
                }
                black_box(&v);
            }
            t.elapsed()
        });
        println!("{:<58} {}", format!("Vec::with_capacity(n) + fill + free (n={n})"), s);

        // Slice::new (validation loop) + drop, includes the Vec above
        let s = sample(SAMPLES, (2_000_000 / n as u64).max(1000), |ops| {
            let t = Instant::now();
            for _ in 0..ops {
                let mut v = Vec::with_capacity(n);
                for i in 0..n {
                    v.push(unsafe { UnownedBuffer::from_raw_parts(base.add(i * 64), 64) });
                }
                let s = Slice::new(v, 0..n * 64, None);
                black_box(&s);
            }
            t.elapsed()
        });
        println!("{:<58} {}", format!("Vec + Slice::new(validation) + drop (n={n})"), s);
    }
    let arc = Arc::new(0u64);
    let s = sample(SAMPLES, 2_000_000, |ops| {
        let t = Instant::now();
        for _ in 0..ops {
            black_box(Arc::clone(&arc));
        }
        t.elapsed()
    });
    println!("{:<58} {}", "Arc::clone + drop", s);
    let m = Mutex::new(0u64);
    let s = sample(SAMPLES, 2_000_000, |ops| {
        let t = Instant::now();
        for _ in 0..ops {
            *m.lock().unwrap() += 1;
        }
        t.elapsed()
    });
    println!("{:<58} {}", "std Mutex lock + unlock (uncontended)", s);
    black_box(&backing);
}

fn alignment(rt: &Runtime) {
    println!("\n## Alignment of the first chunk (address mod 4096 / mod 64)");
    rt.block_on(async {
        for (bs, cnt) in [(4096usize, 32768usize), (65536, 2048), (1 << 20, 128)] {
            let a = upstream(bs, cnt);
            let b = a.allocate(nz(bs)).await.unwrap();
            let p = b.chunks().next().unwrap().as_ptr() as usize;
            println!(
                "upstream Impl(buffer={:>7}): ptr % 4096 = {:>4}, ptr % 64 = {:>2}",
                bs,
                p % 4096,
                p % 64
            );
        }
        let a = buddy(POOL, false);
        let b = a.allocate(nz(PAGE)).await.unwrap();
        let p = b.chunks().next().unwrap().as_ptr() as usize;
        println!(
            "buddy (mmap arena)        : ptr % 4096 = {:>4}, ptr % 64 = {:>2}",
            p % 4096,
            p % 64
        );
    });
}

fn main() {
    let rt = current_thread();
    println!("# micro: allocate + drop, single thread, pool = {} MiB", POOL >> 20);
    for ring in [0usize, 32] {
        println!("\n## ring = {ring} other buffers in flight");
        println!("{:<34} {:>9} {:>40}", "allocator", "request", "time");
        for &size in &[4096usize, 65536, 1 << 20] {
            for &bs in &[4096usize, 65536, 1 << 20] {
                let a = upstream(bs, POOL / bs);
                let s = bench_alloc(&rt, &a, size, ring);
                println!("{:<34} {:>9} {}", format!("upstream Impl(buffer={bs})"), size, s);
            }
            for &bs in &[4096usize, 65536, 1 << 20] {
                let a = fixed(bs, POOL / bs, false);
                let s = bench_alloc(&rt, &a, size, ring);
                println!("{:<34} {:>9} {}", format!("fixed-lifo(buffer={bs})"), size, s);
            }
            for &bs in &[4096usize, 65536, 1 << 20] {
                let a = lockfree(bs, POOL / bs, false);
                let s = bench_alloc(&rt, &a, size, ring);
                println!("{:<34} {:>9} {}", format!("lockfree-lifo(buffer={bs})"), size, s);
            }
            let a = buddy(POOL, false);
            let s = bench_alloc(&rt, &a, size, ring);
            println!("{:<34} {:>9} {}", "buddy(page=4096)", size, s);
            let a = buddy_cached(POOL, false);
            let s = bench_alloc(&rt, &a, size, ring);
            println!("{:<34} {:>9} {}", "buddy+cache(page=4096)", size, s);
        }
    }
    for &size in &[12_288usize, 61_440, 200_000] {
        let a = buddy(POOL, false);
        let s = bench_alloc(&rt, &a, size, 32);
        println!("{:<34} {:>9} {}", "buddy(page=4096), odd size", size, s);
        let a = buddy_cached(POOL, false);
        let s = bench_alloc(&rt, &a, size, 32);
        println!("{:<34} {:>9} {}", "buddy+cache, odd size", size, s);
    }
    breakdown(&rt);
    alignment(&rt);
}
