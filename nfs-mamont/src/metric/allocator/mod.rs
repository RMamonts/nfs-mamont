//! Metrics of the built-in [`crate::allocator::Impl`].

#[cfg(feature = "metrics")]
mod allocator_metrics;
#[cfg(feature = "metrics")]
pub use allocator_metrics::*;

#[cfg(not(feature = "metrics"))]
mod noop;
#[cfg(not(feature = "metrics"))]
pub use noop::*;
