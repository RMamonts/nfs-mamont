//! Defines tests for [`crate::allocator::Allocator::allocate`] interface.

use std::io::Write;
use std::num::NonZeroUsize;
use std::time::Duration;

use crate::allocator::Allocator as _;
use crate::allocator::Impl;

/// `Option::unwrap` is not a `const fn` before Rust 1.83, so constants are built through this
/// helper instead.
const fn nonzero(value: usize) -> NonZeroUsize {
    match NonZeroUsize::new(value) {
        Some(value) => value,
        None => panic!("value must be non-zero"),
    }
}

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

    let allocator_capacity = nonzero(buffer_size.get() * count.get());
    let slice = allocator.allocate(allocator_capacity).await.unwrap();
    assert_eq!(slice.iter().count(), count.get());
}

#[tokio::test]
async fn allocate_less_than_size() {
    const BUFFER_SIZE: NonZeroUsize = nonzero(13);
    const BUFFER_CONT: NonZeroUsize = nonzero(15);

    for alloc_size in 1..BUFFER_SIZE.get() {
        let alloc_size = nonzero(alloc_size);
        check_allocate(BUFFER_SIZE, BUFFER_CONT, alloc_size).await
    }
}

#[tokio::test]
async fn allocate_size() {
    const BUFFER_SIZE: NonZeroUsize = nonzero(13);
    const BUFFER_CONT: NonZeroUsize = nonzero(15);

    check_allocate(BUFFER_SIZE, BUFFER_CONT, BUFFER_SIZE).await
}

#[tokio::test]
async fn allocate_more_than_size() {
    const BUFFER_SIZE: NonZeroUsize = nonzero(13);
    const BUFFER_CONT: NonZeroUsize = nonzero(15);

    for alloc_size in BUFFER_SIZE.get()..BUFFER_SIZE.get() * BUFFER_CONT.get() {
        let alloc_size = nonzero(alloc_size);
        check_allocate(BUFFER_SIZE, BUFFER_CONT, alloc_size).await
    }
}

#[tokio::test]
async fn allocate_capacity() {
    const BUFFER_SIZE: NonZeroUsize = nonzero(13);
    const BUFFER_CONT: NonZeroUsize = nonzero(15);

    let capacity = nonzero(BUFFER_CONT.get() * BUFFER_SIZE.get());
    check_allocate(BUFFER_SIZE, BUFFER_CONT, capacity).await
}

#[tokio::test]
async fn reclaiming() {
    const SIZE: NonZeroUsize = nonzero(13);
    const COUNT: NonZeroUsize = nonzero(15);
    const ALLOC_SIZE: NonZeroUsize = nonzero(SIZE.get() * COUNT.get());

    let allocator = Impl::new(SIZE, COUNT);

    for _ in 0..5 {
        let slice = allocator.allocate(ALLOC_SIZE).await.unwrap();
        assert_eq!(slice.iter().count(), COUNT.get());

        tokio::time::timeout(Duration::from_millis(120), async {
            allocator.allocate(nonzero(1)).await.unwrap();
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
async fn allocate_more_than_capacity_returns_none() {
    const SIZE: NonZeroUsize = nonzero(13);
    const COUNT: NonZeroUsize = nonzero(15);

    let allocator = Impl::new(SIZE, COUNT);
    let requested = nonzero(SIZE.get() * COUNT.get() + 1);

    assert!(allocator.allocate(requested).await.is_none());
}
