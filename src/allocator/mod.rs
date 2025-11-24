mod buffer;
mod list;
mod slice;

use std::num::NonZeroUsize;

use async_trait::async_trait;
use tokio::sync::mpsc;

use buffer::Buffer;
use list::List;

pub use slice::Slice;

type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

/// Allocates [`Slice`]'s.
#[async_trait]
pub trait Allocator {
    /// Allocates [`Slice`] of specified size.
    ///
    /// # Parameters
    ///
    /// - `size` --- size of returnred slice.
    ///
    /// # Panic
    ///
    /// This method panics if size is greater then allocator capacity.
    async fn alloc(&mut self, size: NonZeroUsize) -> Option<slice::Slice>;
}

pub struct Impl {
    receiver: Receiver<Buffer>,
    sender: Sender<Buffer>,
    capacity: usize,
}

impl Impl {
    pub fn new(size: NonZeroUsize, count: NonZeroUsize) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel::<Buffer>();

        for _ in 0..count.get() {
            sender.send(Buffer::new(size)).expect("to init Allocator");
        }

        Self { sender, receiver, capacity: size.get() * count.get() }
    }
}

#[async_trait]
impl Allocator for Impl {
    async fn alloc(&mut self, size: NonZeroUsize) -> Option<slice::Slice> {
        assert!(size.get() <= self.capacity, "cannot allocate more than allocattor capacity");

        let mut remain_size = size.get();
        let mut buffers = Vec::with_capacity(remain_size);

        while remain_size > 0 {
            let buffer = self.receiver.recv().await?;

            remain_size = remain_size.saturating_sub(buffer.len());
            buffers.push(buffer);
        }

        let list = List::new(buffers, self.sender.clone());
        let slice = Slice::new(list, 0..size.get());

        Some(slice)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::num::NonZeroUsize;

    use super::Allocator as _;
    use super::Impl;

    #[tokio::test]
    async fn allocate_less_than_size() {
        const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
        const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();

        let mut allocator = Impl::new(SIZE, COUNT);

        const SLICE_LEN: usize = SIZE.get() - 1;
        let to_write: Vec<u8> = (0..SLICE_LEN).map(|u| (u + 1) as u8).collect();
        let mut slice = allocator.alloc(NonZeroUsize::new(SLICE_LEN).unwrap()).await.unwrap();

        {
            let mut slice_iter = (&mut slice).into_iter();

            let mut buffer = slice_iter.next().unwrap();
            assert_eq!(buffer.len(), SLICE_LEN);
            buffer.write_all(to_write.as_slice()).unwrap();

            assert!(slice_iter.next().is_none());
            assert!(slice_iter.next().is_none());
        }

        let mut slice_iter = (&mut slice).into_iter();

        let buffer = slice_iter.next().unwrap();
        assert_eq!(buffer.len(), SLICE_LEN);
        assert_eq!(buffer, to_write.as_slice());

        assert!(slice_iter.next().is_none());
        assert!(slice_iter.next().is_none());
    }

    #[tokio::test]
    async fn allocate_size() {
        const SIZE: NonZeroUsize = NonZeroUsize::new(13).unwrap();
        const COUNT: NonZeroUsize = NonZeroUsize::new(15).unwrap();

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

        let mut slice_iter = (&mut slice).into_iter();

        let buffer = slice_iter.next().unwrap();
        assert_eq!(buffer.len(), SIZE.get());
        assert_eq!(buffer, to_write.as_slice());

        assert!(slice_iter.next().is_none());
        assert!(slice_iter.next().is_none());
    }
}
