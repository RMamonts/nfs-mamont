//! Correctness tests for the prototype pools (data integrity, waiting, FIFO,
//! cancellation, multi-threaded stress).

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use allocator_bench::{buddy, buddy_cached, fixed, lockfree, PAGE};
use nfs_mamont::{Allocator, Buffer};

fn nz(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}

async fn roundtrip<A: Allocator>(a: &A, size: usize) {
    let mut b = a.allocate(nz(size)).await.unwrap();
    assert_eq!(b.len(), size);
    let mut i = 0usize;
    for c in b.chunks_mut() {
        for x in c.iter_mut() {
            *x = (i * 31 % 251) as u8;
            i += 1;
        }
    }
    assert_eq!(i, size);
    let mut j = 0usize;
    for c in b.chunks() {
        for &x in c {
            assert_eq!(x, (j * 31 % 251) as u8);
            j += 1;
        }
    }
    assert_eq!(j, size);
}

#[tokio::test]
async fn data_integrity() {
    let f = fixed(4096, 64, false);
    let b = buddy(64 * PAGE, false);
    let c = buddy_cached(64 * PAGE, false);
    let l = lockfree(4096, 64, false);
    for size in [1, 100, 4095, 4096, 4097, 12_288, 65_536, 65_537, 200_000, 64 * PAGE] {
        roundtrip(&f, size).await;
        roundtrip(&b, size).await;
        roundtrip(&c, size).await;
        roundtrip(&l, size).await;
    }
    assert!(l.allocate(nz(64 * PAGE + 1)).await.is_none());
    assert_eq!(l.free_bytes(), 64 * 4096);
    assert_eq!(c.free_bytes(), 64 * PAGE);
    assert!(f.allocate(nz(64 * PAGE + 1)).await.is_none());
    assert!(b.allocate(nz(64 * PAGE + 1)).await.is_none());
    assert_eq!(f.free_bytes(), 64 * 4096);
    assert_eq!(b.free_bytes(), 64 * PAGE);
}

#[tokio::test]
async fn buddy_is_contiguous_and_exact() {
    let b = buddy(1024 * PAGE, false);
    for size in [4096, 12_288, 61_440, 65_536, 1 << 20] {
        let buf = b.allocate(nz(size)).await.unwrap();
        assert_eq!(buf.chunk_count(), 1, "size {size}");
        assert_eq!(buf.reserved(), size.div_ceil(PAGE) * PAGE);
        let ptr = buf.chunks().next().unwrap().as_ptr() as usize;
        assert_eq!(ptr % PAGE, 0, "page aligned");
    }
}

#[tokio::test]
async fn waits_and_wakes() {
    let a = Arc::new(buddy(16 * PAGE, false));
    let all = a.allocate(nz(16 * PAGE)).await.unwrap();
    let a2 = Arc::clone(&a);
    let h = tokio::spawn(async move { a2.allocate(nz(PAGE)).await.unwrap().len() });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!h.is_finished());
    assert_eq!(a.waiters(), 1);
    drop(all);
    assert_eq!(h.await.unwrap(), PAGE);
    assert_eq!(a.free_bytes(), 16 * PAGE);
}

#[tokio::test]
async fn fifo_head_of_line() {
    // Like tokio's semaphore: a big request at the head blocks later small ones.
    let a = Arc::new(fixed(PAGE, 4, false));
    let held = a.allocate(nz(3 * PAGE)).await.unwrap();
    let a2 = Arc::clone(&a);
    let big = tokio::spawn(async move { a2.allocate(nz(4 * PAGE)).await.map(|b| b.len()) });
    tokio::time::sleep(Duration::from_millis(10)).await;
    let a3 = Arc::clone(&a);
    let small = tokio::spawn(async move { a3.allocate(nz(PAGE)).await.map(|b| b.len()) });
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(!small.is_finished(), "small request must queue behind the big one");
    drop(held);
    assert_eq!(big.await.unwrap(), Some(4 * PAGE));
    assert_eq!(small.await.unwrap(), Some(PAGE));
}

#[tokio::test]
async fn cancellation_returns_everything() {
    let a = Arc::new(buddy(8 * PAGE, false));
    let all = a.allocate(nz(8 * PAGE)).await.unwrap();
    for _ in 0..10 {
        let r = tokio::time::timeout(Duration::from_millis(1), a.allocate(nz(PAGE))).await;
        assert!(r.is_err());
    }
    assert_eq!(a.waiters(), 0);
    drop(all);
    assert_eq!(a.free_bytes(), 8 * PAGE);
}

async fn stress<A>(a: Arc<A>, max: usize)
where
    A: Allocator + Send + Sync + 'static,
    A::Buffer: 'static,
{
    let mut tasks = Vec::new();
    for t in 0..32u64 {
        let a = Arc::clone(&a);
        tasks.push(tokio::spawn(async move {
            let mut x = 0x2545F4914F6CDD1Du64 ^ (t << 32);
            for _ in 0..2000 {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                let size = 1 + (x as usize % max);
                if x % 7 == 0 {
                    let _ =
                        tokio::time::timeout(Duration::from_micros(50), a.allocate(nz(size))).await;
                } else {
                    let mut b = a.allocate(nz(size)).await.unwrap();
                    if let Some(c) = b.chunks_mut().next() {
                        c[0] = t as u8;
                    }
                    if x % 3 == 0 {
                        tokio::task::yield_now().await;
                    }
                    drop(b);
                }
            }
        }));
    }
    for t in tasks {
        t.await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stress_buddy() {
    let a = Arc::new(buddy(256 * PAGE, false));
    stress(Arc::clone(&a), 80 * PAGE).await;
    assert_eq!(a.waiters(), 0);
    assert_eq!(a.free_bytes(), 256 * PAGE);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stress_fixed() {
    let a = Arc::new(fixed(PAGE, 256, false));
    stress(Arc::clone(&a), 80 * PAGE).await;
    assert_eq!(a.waiters(), 0);
    assert_eq!(a.free_bytes(), 256 * PAGE);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stress_buddy_cached() {
    let a = Arc::new(buddy_cached(256 * PAGE, false));
    stress(Arc::clone(&a), 80 * PAGE).await;
    assert_eq!(a.waiters(), 0);
    assert_eq!(a.free_bytes(), 256 * PAGE);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stress_lockfree() {
    for _ in 0..5 {
        let a = Arc::new(lockfree(PAGE, 256, false));
        stress(Arc::clone(&a), 80 * PAGE).await;
        assert_eq!(a.waiters(), 0);
        assert_eq!(a.free_bytes(), 256 * PAGE);
        // every block is back exactly once
        let all = a.allocate(nz(256 * PAGE)).await.unwrap();
        let mut seen = std::collections::HashSet::new();
        for c in all.chunks() {
            assert!(seen.insert(c.as_ptr() as usize), "block handed out twice");
        }
        assert_eq!(seen.len(), 256);
    }
}

#[tokio::test]
async fn lockfree_waits_fifo_and_cancels() {
    let a = Arc::new(lockfree(PAGE, 4, false));
    let held = a.allocate(nz(3 * PAGE)).await.unwrap();
    let a2 = Arc::clone(&a);
    let big = tokio::spawn(async move { a2.allocate(nz(4 * PAGE)).await.map(|b| b.len()) });
    tokio::time::sleep(Duration::from_millis(10)).await;
    let a3 = Arc::clone(&a);
    let small = tokio::spawn(async move { a3.allocate(nz(PAGE)).await.map(|b| b.len()) });
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(!small.is_finished());
    for _ in 0..5 {
        assert!(tokio::time::timeout(Duration::from_millis(1), a.allocate(nz(PAGE)))
            .await
            .is_err());
    }
    drop(held);
    assert_eq!(big.await.unwrap(), Some(4 * PAGE));
    assert_eq!(small.await.unwrap(), Some(PAGE));
    assert_eq!(a.waiters(), 0);
    assert_eq!(a.free_bytes(), 4 * PAGE);
}
