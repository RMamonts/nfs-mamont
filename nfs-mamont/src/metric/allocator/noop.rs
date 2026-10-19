/// No-op allocator metrics: the `metrics` feature is disabled.

#[derive(Debug)]
pub struct AllocatorMetrics;

/// No-op allocation timer.
pub struct AllocTimer;

impl AllocatorMetrics {
    pub fn new(_chunks: usize, _size: usize) -> Self {
        Self
    }

    #[inline]
    pub fn start(&self) -> AllocTimer {
        AllocTimer
    }

    #[inline]
    pub fn allocated(&self, _timer: AllocTimer, _size: usize, _chunks: usize) {}

    #[inline]
    pub fn rejected(&self) {}

    #[inline]
    pub fn released(&self, _chunks: usize) {}
}
