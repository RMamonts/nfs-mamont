use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::uring::executor::UringExecutor;

#[derive(Debug)]
pub struct UringPool {
    rings: Vec<Arc<UringExecutor>>,
    next: AtomicUsize,
}

impl UringPool {
    pub fn new(count: usize, entries: u32) -> Option<Arc<Self>> {
        if count == 0 {
            return None;
        }

        let mut rings = Vec::with_capacity(count);
        for _ in 0..count {
            rings.push(UringExecutor::new(entries)?);
        }

        Some(Arc::new(Self { rings, next: AtomicUsize::new(0) }))
    }

    pub fn pick(&self) -> Arc<UringExecutor> {
        let index = self.next.fetch_add(1, Ordering::Relaxed) % self.rings.len();
        self.rings[index].clone()
    }

    pub fn max_io_len(&self) -> usize {
        self.rings.first().map(|ring| ring.max_io_len()).unwrap_or(usize::MAX)
    }
}
