//! Defines [`Allocator`] interface used to bound allocation of buffers
//! for user data transmission inside NFS-Mamont implementation.

mod slice;

use std::num::NonZeroUsize;

use async_trait::async_trait;
use tokio::sync::mpsc;

pub use slice::Slice;

type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;
type Buffer = Box<[u8]>;

/// Allocates [`Slice`]'s.
#[async_trait]
pub trait Allocator {
    /// Returns [`Slice`] of specified size.
    ///
    /// # Parameters
    ///
    /// - `size` --- size of returned slice.
    ///
    /// # Panic
    ///
    /// This method panics if size is greater then allocator capacity.
    async fn alloc(&mut self, size: NonZeroUsize) -> Option<slice::Slice>;
}

pub struct Impl {
    receiver: Receiver<Buffer>,
    sender: Sender<Buffer>,
    buffer_size: NonZeroUsize,
    buffer_count: NonZeroUsize,
}

impl Impl {
    /// Returns new [`Allocator`] IMPlementation.
    ///
    /// # Parameters
    ///
    /// - `size` --- size of each buffer to allocate
    /// - `count` --- number of buffers to allocate
    pub fn new(size: NonZeroUsize, count: NonZeroUsize) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel::<Buffer>();

        for _ in 0..count.get() {
            sender.send(vec![0; size.get()].into_boxed_slice()).expect("to init Allocator");
        }

        Self { sender, receiver, buffer_size: size, buffer_count: count }
    }
}

#[async_trait]
impl Allocator for Impl {
    async fn alloc(&mut self, size: NonZeroUsize) -> Option<slice::Slice> {
        assert!(
            size.get() <= self.buffer_size.get() * self.buffer_count.get(),
            "cannot allocate more than allocattor capacity"
        );

        let mut remain_size = size.get();
        let mut buffers = Vec::with_capacity(remain_size.div_ceil(self.buffer_size.get()));

        while remain_size > 0 {
            let buffer = self.receiver.recv().await?;
            assert_eq!(buffer.len(), self.buffer_size.get());

            remain_size = remain_size.saturating_sub(buffer.len());
            buffers.push(buffer);
        }

        Some(Slice::new(buffers, 0..size.get(), self.sender.clone()))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::num::NonZeroUsize;
    use std::time::Duration;

    use super::Allocator as _;
    use super::Impl;

    async fn check_allocate(
        buffer_size: NonZeroUsize,
        count: NonZeroUsize,
        alloc_size: NonZeroUsize,
    ) {
        let mut allocator = Impl::new(buffer_size, count);
        let mut slice = allocator.alloc(alloc_size).await.unwrap();

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
        let slice = allocator.alloc(allocator_capacity).await.unwrap();
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
            let slice = allocator.alloc(ALLOC_SIZE).await.unwrap();
            assert_eq!(slice.iter().count(), COUNT.get());
            assert!(slice.iter().all(|buffer| buffer.iter().all(|&u| u == 0)));

            tokio::time::timeout(Duration::from_millis(120), async {
                allocator.alloc(NonZeroUsize::new(1).unwrap()).await.unwrap();
                unreachable!("allocator should hang")
            })
            .await
            .unwrap_err();

            drop(slice);

            let slice = allocator.alloc(ALLOC_SIZE).await.unwrap();
            assert_eq!(slice.iter().count(), COUNT.get());
            assert!(slice.iter().all(|buffer| buffer.iter().all(|&u| u == 0)));
        }
    }
}
