//! Metrics of the built-in [`PoolAllocator`](super::PoolAllocator).

#[cfg(feature = "metrics")]
mod allocator_metrics;
#[cfg(feature = "metrics")]
pub use allocator_metrics::*;
#[cfg(all(test, feature = "metrics"))]
mod test;
#[cfg(all(test, feature = "metrics"))]
mod test_helpers;

#[cfg(not(feature = "metrics"))]
mod noop;
#[cfg(not(feature = "metrics"))]
pub use noop::*;
