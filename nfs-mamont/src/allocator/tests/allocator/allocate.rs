//! Defines tests for [`crate::allocator::Allocator::allocate`] interface.

use std::io::Write;
use std::num::NonZeroUsize;
use std::time::Duration;

use crate::allocator::Allocator as _;
use crate::allocator::Impl;

async fn check_allocate(buffer_size: NonZeroUsize, count: NonZeroUsize, alloc_size: NonZeroUsize) {
    let allocator = Impl::new(buffer_size, count);
    let mut slice = allocator.allocate(alloc_size).await.unwrap();

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
    let slice = allocator.allocate(allocator_capacity).await.unwrap();
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
        let slice = allocator.allocate(ALLOC_SIZE).await.unwrap();
        assert_eq!(slice.iter().count(), COUNT.get());

        tokio::time::timeout(Duration::from_millis(120), async {
            allocator.allocate(NonZeroUsize::new(1).unwrap()).await.unwrap();
            unreachable!("allocator should hang")
        })
        .await
        .unwrap_err();

        drop(slice);

        let slice = allocator.allocate(ALLOC_SIZE).await.unwrap();
        assert_eq!(slice.iter().count(), COUNT.get());
    }
}

#[tokio::test]
async fn try_allocate_more_than_capacity_returns_none() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();

    let allocator = Impl::new(SIZE, COUNT);
    let requested = NonZeroUsize::new(SIZE.get() * COUNT.get() + 1).unwrap();

    assert!(allocator.try_allocate(requested).is_none());

    // The convenience `allocate` falls back to a single block instead of `None`.
    let slice = allocator.allocate(requested).await.unwrap();
    assert_eq!(slice.len(), SIZE.get());
    assert_eq!(slice.iter().count(), 1);
}

#[tokio::test]
async fn try_allocate_returns_none_when_pool_is_exhausted() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(3).unwrap();

    let allocator = Impl::new(SIZE, COUNT);

    // Occupy the entire pool.
    let pool =
        allocator.allocate(NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap()).await.unwrap();

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

    // The permit is restored after drop, so the pool can be fully saturated again.
    let slice =
        allocator.allocate(NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap()).await.unwrap();
    assert_eq!(slice.iter().count(), COUNT.get());
}

#[tokio::test]
async fn allocate_block_blocks_until_a_block_is_released() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();

    let allocator = Impl::new(SIZE, COUNT);

    // Occupy the entire pool.
    let pool =
        allocator.allocate(NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap()).await.unwrap();

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
