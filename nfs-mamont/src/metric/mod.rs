//! Server metrics recorded through the [`metrics`](https://docs.rs/metrics) facade.
//!
//! With the `metrics` feature disabled, [`NfsMetrics`] and its helpers are zero-sized
//! no-ops, so they should be compiled away.
//!
//! Metrics, especially from tokio, rely heavily on timers, so clocksource matters.
//! Kernel clocksource other than `tsc`  could increase the measurement overhead
//! (check `/sys/devices/system/clocksource/clocksource0/current_clocksource`).

mod allocator;
mod kinds;
mod nfs;
mod procedure;

#[cfg(all(test, feature = "metrics"))]
mod test_helpers;

pub use allocator::AllocatorMetrics;
pub use kinds::{Stage, TaskKind};
pub use nfs::*;
