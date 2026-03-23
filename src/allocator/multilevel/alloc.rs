use std::cmp::min;
use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::allocator::multilevel::slice::MultiSlice;
use crate::allocator::{Allocator, Impl};

pub trait MultiAllocator {
    async fn allocate_multi(&mut self, size: NonZeroUsize) -> Option<MultiSlice>;
}

pub struct Level {
    local: Impl,
    upper: Option<Arc<Mutex<Level>>>,
}

impl Level {
    fn new(size: NonZeroUsize, count: NonZeroUsize, upper: Option<Arc<Mutex<Level>>>) -> Self {
        Self { local: Impl::new(size, count), upper }
    }
}

impl MultiAllocator for Level {
    async fn allocate_multi(&mut self, size: NonZeroUsize) -> Option<MultiSlice> {
        let cur_level = min(size.get(), self.local.capacity());
        let from_current = NonZeroUsize::new(cur_level).unwrap_or(NonZeroUsize::MIN);
        let current = self.local.allocate(from_current).await?;

        let remain = size.get().checked_sub(cur_level)?;
        if remain == 0 {
            return Some(MultiSlice::One(current));
        }

        let upper = self.upper.as_ref()?;
        let mut upper_locked = upper.lock().await;

        let rest = match NonZeroUsize::new(remain) {
            Some(rest) => rest,
            None => return None,
        };
        let upper_slice = Box::pin(upper_locked.allocate_multi(rest)).await?;

        Some(MultiSlice::Cons(current, Box::new(upper_slice)))
    }
}
