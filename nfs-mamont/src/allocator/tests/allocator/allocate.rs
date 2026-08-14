//! Defines tests for [`crate::allocator::Allocator::allocate`] interface.

use std::io::Write;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use crate::allocator::Allocator as _;
use crate::allocator::Impl;

async fn check_allocate(buffer_size: NonZeroUsize, count: NonZeroUsize, alloc_size: NonZeroUsize) {
    let allocator = Impl::new(buffer_size, count);
    let mut slice = allocator.allocate(alloc_size).await;

    let verify: Vec<u8> = (0..alloc_size.get()).map(|u| (u + 1) as u8).collect();

    {
        let mut slice_iter = (&mut slice).into_iter();
        for verify_chunk in verify.chunks(buffer_size.get()) {
            let mut buffer = slice_iter.next().unwrap();
            assert_eq!(buffer.len(), verify_chunk.len());
            buffer.write_all(verify_chunk).unwrap();
        }

        assert!(slice_iter.next().is_none());
        assert!(slice_iter.next().is_none());
    }

    {
        let mut slice_iter = (&mut slice).into_iter();
        for verify_chunk in verify.chunks(buffer_size.get()) {
            let buffer = slice_iter.next().unwrap();
            assert_eq!(buffer.len(), verify_chunk.len());
            assert!(buffer == verify_chunk);
        }

        assert!(slice_iter.next().is_none());
        assert!(slice_iter.next().is_none());
    }

    drop(slice);

    let allocator_capacity = NonZeroUsize::new(buffer_size.get() * count.get()).unwrap();
    let slice = allocator.allocate(allocator_capacity).await;
    assert_eq!(slice.iter().count(), count.get());
}

#[tokio::test]
async fn allocate_less_than_size() {
    const BUFFER_SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const BUFFER_CONT: NonZeroUsize = NonZeroUsize::new(15).unwrap();

    for alloc_size in 1..BUFFER_SIZE.get() {
        let alloc_size = NonZeroUsize::new(alloc_size).unwrap();
        check_allocate(BUFFER_SIZE, BUFFER_CONT, alloc_size).await
    }
}

#[tokio::test]
async fn allocate_size() {
    const BUFFER_SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const BUFFER_CONT: NonZeroUsize = NonZeroUsize::new(15).unwrap();

    check_allocate(BUFFER_SIZE, BUFFER_CONT, BUFFER_SIZE).await
}

#[tokio::test]
async fn allocate_more_than_size() {
    const BUFFER_SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const BUFFER_CONT: NonZeroUsize = NonZeroUsize::new(15).unwrap();

    for alloc_size in BUFFER_SIZE.get()..BUFFER_SIZE.get() * BUFFER_CONT.get() {
        let alloc_size = NonZeroUsize::new(alloc_size).unwrap();
        check_allocate(BUFFER_SIZE, BUFFER_CONT, alloc_size).await
    }
}

#[tokio::test]
async fn allocate_capacity() {
    const BUFFER_SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const BUFFER_CONT: NonZeroUsize = NonZeroUsize::new(15).unwrap();

    let capacity = NonZeroUsize::new(BUFFER_CONT.get() * BUFFER_SIZE.get()).unwrap();
    check_allocate(BUFFER_SIZE, BUFFER_CONT, capacity).await
}

#[tokio::test]
async fn reclaiming() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();
    const ALLOC_SIZE: NonZeroUsize = NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap();

    let allocator = Impl::new(SIZE, COUNT);

    for _ in 0..5 {
        let slice = allocator.allocate(ALLOC_SIZE).await;
        assert_eq!(slice.iter().count(), COUNT.get());

        tokio::time::timeout(Duration::from_millis(120), async {
            allocator.allocate(NonZeroUsize::new(1).unwrap()).await;
            unreachable!("allocator should hang")
        })
        .await
        .unwrap_err();

        drop(slice);

        let slice = allocator.allocate(ALLOC_SIZE).await;
        assert_eq!(slice.iter().count(), COUNT.get());
    }
}

#[tokio::test]
async fn try_allocate_more_than_capacity_is_clamped() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();

    let allocator = Impl::new(SIZE, COUNT);
    let requested = NonZeroUsize::new(SIZE.get() * COUNT.get() + 1).unwrap();

    // A request beyond the pool is served with the whole pool rather than
    // rejected --- the caller would only fall back to a single block.
    let slice = allocator.try_allocate(requested).expect("clamped to capacity");
    assert_eq!(slice.len(), SIZE.get() * COUNT.get());
    assert_eq!(slice.iter().count(), COUNT.get());
}

#[tokio::test]
async fn try_allocate_grants_what_the_pool_can_spare() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(8).unwrap();

    let allocator = Impl::new(SIZE, COUNT);

    // Hold three of the eight blocks.
    let held = allocator.try_allocate(NonZeroUsize::new(SIZE.get() * 3).unwrap()).unwrap();
    assert_eq!(held.iter().count(), 3);

    // A request for the whole pool is served partially with the remaining five
    // blocks instead of failing outright.
    let partial = allocator.try_allocate(NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap());
    let partial = partial.expect("partial allocation");
    assert_eq!(partial.len(), SIZE.get() * 5);
    assert_eq!(partial.iter().count(), 5);

    // Now nothing is left.
    assert!(allocator.try_allocate(NonZeroUsize::MIN).is_none());

    drop(partial);
    drop(held);

    assert_eq!(
        allocator.try_allocate(NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap()).unwrap().len(),
        SIZE.get() * COUNT.get()
    );
}

#[tokio::test]
async fn try_allocate_partial_is_bounded_by_the_request() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(8).unwrap();

    let allocator = Impl::new(SIZE, COUNT);

    // Hold everything but two blocks, then ask for a size that needs three:
    // the granted slice spans two blocks but is never longer than requested.
    let held = allocator.try_allocate(NonZeroUsize::new(SIZE.get() * 6).unwrap()).unwrap();
    assert_eq!(held.iter().count(), 6);

    let requested = NonZeroUsize::new(SIZE.get() * 2 + 1).unwrap();
    let partial = allocator.try_allocate(requested).expect("partial allocation");
    assert_eq!(partial.len(), SIZE.get() * 2);
    assert_eq!(partial.iter().count(), 2);
}

#[tokio::test]
async fn try_allocate_returns_none_when_pool_is_exhausted() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(3).unwrap();

    let allocator = Impl::new(SIZE, COUNT);

    // Occupy the entire pool.
    let pool = allocator.allocate(NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap()).await;

    assert!(allocator.try_allocate(NonZeroUsize::MIN).is_none());

    drop(pool);

    // After the buffer is released the pool is available again.
    assert!(allocator.try_allocate(NonZeroUsize::MIN).is_some());
}

#[tokio::test]
async fn allocate_block_returns_single_block() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();

    let allocator = Impl::new(SIZE, COUNT);

    let slice = allocator.allocate_block().await;
    assert_eq!(slice.len(), SIZE.get());
    assert_eq!(slice.iter().count(), 1);

    drop(slice);

    // The block is back in the pool after drop, so it can be fully saturated again.
    let slice = allocator.allocate(NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap()).await;
    assert_eq!(slice.iter().count(), COUNT.get());
}

/// `try_allocate` shares the pool with `allocate_block`, and its non-blocking
/// take bypasses the wake-up queue, so it has to leave a block behind for every
/// caller already parked --- otherwise a busy caller starves a blocked one.
#[tokio::test]
async fn try_allocate_leaves_a_block_for_a_parked_caller() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(2).unwrap();

    let allocator = Arc::new(Impl::new(SIZE, COUNT));

    // Occupy both blocks with separate slices, so one can be released alone.
    let first = allocator.try_allocate(SIZE).unwrap();
    let second = allocator.try_allocate(SIZE).unwrap();

    // Park a caller on the drained pool.
    let parked = tokio::spawn({
        let allocator = Arc::clone(&allocator);
        async move { allocator.allocate_block().await }
    });
    // One scheduler turn is enough for the spawned task to reach its `await`
    // and register; the rest are slack.
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }

    // Hand back a single block. It belongs to the parked caller, and this task
    // does not await in between, so nothing else could have taken it yet.
    drop(first);

    assert!(
        allocator.try_allocate(SIZE).is_none(),
        "try_allocate took the block reserved for the parked caller"
    );

    let slice = tokio::time::timeout(Duration::from_secs(5), parked)
        .await
        .expect("the parked caller was starved")
        .unwrap();
    assert_eq!(slice.len(), SIZE.get());

    // With nobody parked any more, the remaining block is grantable again.
    drop(second);
    assert!(allocator.try_allocate(SIZE).is_some());
}

/// Dropping an `allocate_block` future mid-wait --- what a cancelled connection
/// task does --- must release the block it had reserved.
#[tokio::test]
async fn cancelled_allocate_block_releases_its_reservation() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(4).unwrap();
    const WHOLE_POOL: NonZeroUsize = NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap();

    let allocator = Impl::new(SIZE, COUNT);

    let pool = allocator.allocate(WHOLE_POOL).await;

    {
        let mut parked = Box::pin(allocator.allocate_block());
        // Poll once so the caller registers as waiting, then drop it unfinished.
        assert!(tokio::time::timeout(Duration::from_millis(120), &mut parked).await.is_err());
    }

    drop(pool);

    // With the reservation gone the whole pool is grantable again.
    let slice = allocator.try_allocate(WHOLE_POOL).expect("the pool is free");
    assert_eq!(slice.iter().count(), COUNT.get());
}

/// Many callers contending for a pool far smaller than the demand: every one of
/// them must eventually be served, whichever entry point they use.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_caller_is_eventually_served_under_contention() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(64).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(4).unwrap();
    const TASKS: usize = 64;

    let allocator = Arc::new(Impl::new(SIZE, COUNT));

    let tasks: Vec<_> = (0..TASKS)
        .map(|n| {
            let allocator = Arc::clone(&allocator);
            tokio::spawn(async move {
                for _ in 0..16 {
                    // Half the tasks ask for more than the pool holds, half for
                    // a single block; both must make progress.
                    let want = if n % 2 == 0 { SIZE.get() * COUNT.get() } else { SIZE.get() };
                    let slice = allocator.allocate(NonZeroUsize::new(want).unwrap()).await;
                    assert!(!slice.is_empty());
                    tokio::task::yield_now().await;
                    drop(slice);
                }
            })
        })
        .collect();

    for task in tasks {
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .expect("a caller was starved")
            .expect("a caller panicked");
    }

    // Nothing leaked: the pool is whole again.
    let slice = allocator
        .try_allocate(NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap())
        .expect("every block came back");
    assert_eq!(slice.iter().count(), COUNT.get());
}

#[tokio::test]
async fn allocate_block_blocks_until_a_block_is_released() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();

    let allocator = Impl::new(SIZE, COUNT);

    // Occupy the entire pool.
    let pool = allocator.allocate(NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap()).await;

    let (tx, mut rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(async move {
        let slice = allocator.allocate_block().await;
        let _ = tx.send(slice);
    });

    // The task cannot complete while the pool is fully held.
    assert!(tokio::time::timeout(Duration::from_millis(120), &mut rx).await.is_err());

    drop(pool);

    let slice = tokio::time::timeout(Duration::from_secs(5), rx)
        .await
        .expect("timed out waiting for block")
        .unwrap();
    assert_eq!(slice.len(), SIZE.get());
    assert_eq!(slice.iter().count(), 1);
    assert!(handle.await.is_ok());
}
