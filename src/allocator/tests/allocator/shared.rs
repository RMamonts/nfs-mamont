use std::num::NonZeroUsize;
use std::time::Duration;

use crate::allocator::{Allocator as _, Impl, SharedAllocator};

#[tokio::test]
async fn clones_share_one_buffer_pool() {
    const SIZE: NonZeroUsize = NonZeroUsize::new(8).unwrap();
    const COUNT: NonZeroUsize = NonZeroUsize::new(1).unwrap();

    let mut first = SharedAllocator::new(Impl::new(SIZE, COUNT));
    let mut second = first.clone();

    let slice = first.allocate(SIZE).await.unwrap();

    tokio::time::timeout(Duration::from_millis(120), async {
        second.allocate(NonZeroUsize::MIN).await.unwrap();
        unreachable!("shared allocator should wait for a buffer to be returned")
    })
    .await
    .unwrap_err();

    drop(slice);

    let slice = second.allocate(SIZE).await.unwrap();
    assert_eq!(slice.iter().map(|buffer| buffer.len()).sum::<usize>(), SIZE.get());
}
