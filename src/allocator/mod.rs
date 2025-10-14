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

    #[tokio::test]
    async fn allocate_less_than_size() {
        const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
        const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();
        const LESS_THEN_ONE: usize = SIZE.get() - 1;
        const ALL: NonZeroUsize = NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap();

        let mut allocator = Impl::new(SIZE, COUNT);

        let to_write: Vec<u8> = (0..LESS_THEN_ONE).map(|u| (u + 1) as u8).collect();
        let mut slice = allocator.alloc(NonZeroUsize::new(LESS_THEN_ONE).unwrap()).await.unwrap();

        {
            let mut slice_iter = (&mut slice).into_iter();

            let mut buffer = slice_iter.next().unwrap();
            assert_eq!(buffer.len(), LESS_THEN_ONE);
            buffer.write_all(to_write.as_slice()).unwrap();

            assert!(slice_iter.next().is_none());
            assert!(slice_iter.next().is_none());
        }

        {
            let mut slice_iter = (&mut slice).into_iter();

            let buffer = slice_iter.next().unwrap();
            assert_eq!(buffer.len(), LESS_THEN_ONE);
            assert_eq!(buffer, to_write.as_slice());

            assert!(slice_iter.next().is_none());
            assert!(slice_iter.next().is_none());
        }

        drop(slice);

        let slice = allocator.alloc(ALL).await.unwrap();
        assert_eq!(slice.iter().count(), COUNT.get());
        assert!(slice.iter().all(|buffer| buffer.iter().all(|&u| u == 0)));
    }

    #[tokio::test]
    async fn allocate_size() {
        const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
        const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();
        const ALL: NonZeroUsize = NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap();

        let mut allocator = Impl::new(SIZE, COUNT);
        let to_write: Vec<u8> = (0..SIZE.get()).map(|u| u as u8).collect();
        let mut slice = allocator.alloc(SIZE).await.unwrap();

        {
            let mut slice_iter = (&mut slice).into_iter();

            let mut buffer = slice_iter.next().unwrap();
            assert_eq!(buffer.len(), SIZE.get());
            buffer.write_all(to_write.as_slice()).unwrap();

            assert!(slice_iter.next().is_none());
            assert!(slice_iter.next().is_none());
        }

        {
            let mut slice_iter = (&mut slice).into_iter();

            let buffer = slice_iter.next().unwrap();
            assert_eq!(buffer.len(), SIZE.get());
            assert_eq!(buffer, to_write.as_slice());

            assert!(slice_iter.next().is_none());
            assert!(slice_iter.next().is_none());
        }

        drop(slice);

        let slice = allocator.alloc(ALL).await.unwrap();
        assert_eq!(slice.iter().count(), COUNT.get());
        assert!(slice.iter().all(|buffer| buffer.iter().all(|&u| u == 0)));
    }

    #[tokio::test]
    async fn allocate_more_then_size() {
        const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
        const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();
        const MORE_THEN_ONE: NonZeroUsize = NonZeroUsize::new(SIZE.get() + 1).unwrap();
        const ALL: NonZeroUsize = NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap();

        let mut allocator = Impl::new(SIZE, COUNT);
        let mut slice = allocator.alloc(MORE_THEN_ONE).await.unwrap();

        let to_write: Vec<u8> = (0..MORE_THEN_ONE.get()).map(|u| u as u8).collect();

        {
            let mut slice_iter = (&mut slice).into_iter();

            let mut first_buffer = slice_iter.next().unwrap();
            assert_eq!(first_buffer.len(), SIZE.get());
            assert!(first_buffer.iter().all(|&u| u == 0));

            let mut second_buffer = slice_iter.next().unwrap();
            assert_eq!(second_buffer.len(), MORE_THEN_ONE.get() - SIZE.get());
            assert!(second_buffer.iter().all(|&u| u == 0));

            assert!(slice_iter.next().is_none());
            assert!(slice_iter.next().is_none());

            first_buffer.write_all(&to_write.as_slice()[..SIZE.get()]).unwrap();
            second_buffer.write_all(&to_write.as_slice()[SIZE.get()..]).unwrap();
        }

        {
            let mut slice_iter = (&mut slice).into_iter();

            let first_buffer = slice_iter.next().unwrap();
            assert_eq!(first_buffer.len(), SIZE.get());
            assert_eq!(first_buffer, &to_write.as_slice()[..SIZE.get()]);

            let second_buffer = slice_iter.next().unwrap();
            assert_eq!(second_buffer.len(), MORE_THEN_ONE.get() - SIZE.get());
            assert_eq!(second_buffer, &to_write.as_slice()[SIZE.get()..]);

            assert!(slice_iter.next().is_none());
            assert!(slice_iter.next().is_none());
        }

        drop(slice);

        let slice = allocator.alloc(ALL).await.unwrap();
        assert_eq!(slice.iter().count(), COUNT.get());
        assert!(slice.iter().all(|buffer| buffer.iter().all(|&u| u == 0)));
    }

    #[tokio::test]
    async fn allocate_capacity() {
        const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
        const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();
        const ALLOC_SIZE: NonZeroUsize = NonZeroUsize::new(SIZE.get() * COUNT.get()).unwrap();

        let mut allocator = Impl::new(SIZE, COUNT);

        let mut slice = allocator.alloc(ALLOC_SIZE).await.unwrap();
        assert_eq!(slice.iter().count(), COUNT.get());
        assert!(slice.iter().all(|buffer| buffer.iter().all(|&u| u == 0)));

        let to_verify: Vec<u8> = (0..ALLOC_SIZE.get()).map(|u| (u + 1) as u8).collect();
        for (idx, mut buffer) in slice.iter_mut().enumerate() {
            buffer.write_all(&to_verify[idx * SIZE.get()..(idx + 1) * SIZE.get()]).unwrap()
        }

        let to_verify: Vec<u8> = (0..ALLOC_SIZE.get()).map(|u| (u + 1) as u8).collect();
        for (idx, buffer) in slice.iter_mut().enumerate() {
            assert_eq!(buffer, &to_verify[idx * SIZE.get()..(idx + 1) * SIZE.get()])
        }

        drop(slice);

        let slice = allocator.alloc(ALLOC_SIZE).await.unwrap();
        assert_eq!(slice.iter().count(), COUNT.get());
        assert!(slice.iter().all(|buffer| buffer.iter().all(|&u| u == 0)));
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
