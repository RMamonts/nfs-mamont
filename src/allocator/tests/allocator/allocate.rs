//! Defines tests for [`crate::allocator::Allocator::allocate`] interface.

use std::io::Write;
use std::num::NonZeroUsize;
use std::time::Duration;

use crate::allocator::Allocator as _;
use crate::allocator::{Impl, MemoryBudget};

async fn check_allocate(buffer_size: NonZeroUsize, count: NonZeroUsize, alloc_size: NonZeroUsize) {
    let mut allocator = Impl::new(buffer_size, count);
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
    assert!(slice.iter().all(|buffer| buffer.iter().all(|&u| u == 0)));
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

    let mut allocator = Impl::new(SIZE, COUNT);

    for _ in 0..5 {
        let slice = allocator.allocate(ALLOC_SIZE).await.unwrap();
        assert_eq!(slice.iter().count(), COUNT.get());
        assert!(slice.iter().all(|buffer| buffer.iter().all(|&u| u == 0)));

        tokio::time::timeout(Duration::from_millis(120), async {
            allocator.allocate(NonZeroUsize::new(1).unwrap()).await.unwrap();
            unreachable!("allocator should hang")
        })
        .await
        .unwrap_err();

        drop(slice);

        let slice = allocator.allocate(ALLOC_SIZE).await.unwrap();
        assert_eq!(slice.iter().count(), COUNT.get());
        assert!(slice.iter().all(|buffer| buffer.iter().all(|&u| u == 0)));
    }
}

#[tokio::test]
async fn allocate_more_than_capacity_returns_none() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();

    let mut allocator = Impl::new(SIZE, COUNT);
    let requested = NonZeroUsize::new(SIZE.get() * COUNT.get() + 1).unwrap();

    assert!(allocator.allocate(requested).await.is_none());
}

#[tokio::test]
async fn shared_budget_blocks_other_allocators_until_release() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(8).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(2).unwrap();
    const REQUEST: NonZeroUsize = NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap();

    let budget = MemoryBudget::new(COUNT);
    let mut first = Impl::with_budget(SIZE, COUNT, budget.clone());
    let mut second = Impl::with_budget(SIZE, COUNT, budget);

    let slice = first.allocate(REQUEST).await.unwrap();
    assert_eq!(slice.iter().count(), COUNT.get());

    tokio::time::timeout(Duration::from_millis(120), async {
        second.allocate(NonZeroUsize::new(1).unwrap()).await.unwrap();
        unreachable!("shared budget should block until buffers are released")
    })
    .await
    .unwrap_err();

    drop(slice);

    let slice = second.allocate(NonZeroUsize::new(1).unwrap()).await.unwrap();
    assert_eq!(slice.iter().count(), 1);
}

#[tokio::test]
async fn global_budget_can_be_tighter_than_local_pool() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(8).unwrap();
    const LOCAL_COUNT: NonZeroUsize = NonZeroUsize::new(2).unwrap();
    const GLOBAL_BUDGET: NonZeroUsize = NonZeroUsize::new(1).unwrap();

    let budget = MemoryBudget::new(GLOBAL_BUDGET);
    let mut allocator = Impl::with_budget(SIZE, LOCAL_COUNT, budget.clone());

    let slice = allocator.allocate(NonZeroUsize::new(1).unwrap()).await.unwrap();
    assert_eq!(slice.iter().count(), 1);
    assert_eq!(budget.available(), 0);

    tokio::time::timeout(Duration::from_millis(120), async {
        allocator.allocate(SIZE).await.unwrap();
        unreachable!("global budget should apply even when local buffers exist")
    })
    .await
    .unwrap_err();

    drop(slice);
    assert_eq!(budget.available(), GLOBAL_BUDGET.get());
}
