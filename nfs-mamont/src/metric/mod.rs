//! Server metrics recorded through the [`metrics`](https://docs.rs/metrics) facade.
//!
//! With the `metrics` feature disabled, [`NfsMetrics`] and its helpers are zero-sized
//! no-ops, so they should be compiled away.
//!
//! Metrics, especially from tokio, rely heavily on timers, so clocksource matters.
//! Kernel clocksource other than `tsc`  could increase the measurement overhead
//! (check `/sys/devices/system/clocksource/clocksource0/current_clocksource`).

mod kinds;
mod nfs;
mod procedure;
mod queue;

#[cfg(all(test, feature = "metrics"))]
mod test_helpers;

pub use kinds::{Queue, Stage, TaskKind};
pub use nfs::*;
pub use queue::{unbounded, CountedReceiver, CountedSender};
