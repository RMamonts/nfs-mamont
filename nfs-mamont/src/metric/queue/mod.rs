//! Channels whose depth is tracked as a [`Queue`](super::Queue) gauge.
//!
//! With the `metrics` feature disabled, these are plain `async_channel` types.

#[cfg(feature = "metrics")]
mod counted;
#[cfg(feature = "metrics")]
pub use counted::*;
#[cfg(all(test, feature = "metrics"))]
mod test;

#[cfg(not(feature = "metrics"))]
mod noop;
#[cfg(not(feature = "metrics"))]
pub use noop::*;
