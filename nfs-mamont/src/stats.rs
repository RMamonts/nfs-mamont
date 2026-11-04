//! Counters of the server budgets, for benchmarks and diagnostics.
//!
//! The counters are process-wide and only touched on slow paths (a task that has to
//! wait, a timeout, a scheduler round), so they cost nothing on the hot path. Read
//! them with [`snapshot`].

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Times a read task parked on the in-flight budget of its connection.
pub static IN_FLIGHT_WAITS: AtomicU64 = AtomicU64::new(0);
/// Times a read task parked on the data limit of its connection.
pub static CONNECTION_DATA_WAITS: AtomicU64 = AtomicU64::new(0);
/// Times a read task parked on the global data budget.
pub static DATA_BUDGET_WAITS: AtomicU64 = AtomicU64::new(0);
/// Times a read task parked on the global pending budget.
pub static PENDING_WAITS: AtomicU64 = AtomicU64::new(0);
/// Times the accept loop parked on the connection budget.
pub static CONNECTION_WAITS: AtomicU64 = AtomicU64::new(0);
/// Times the allocator itself had to wait, i.e. the budgets did not keep it from
/// running dry.
pub static ALLOCATOR_WAITS: AtomicU64 = AtomicU64::new(0);
/// Total time the allocator itself spent waiting, in nanoseconds.
pub static ALLOCATOR_WAIT_NS: AtomicU64 = AtomicU64::new(0);
/// Connections closed because the client stopped reading its replies.
pub static WRITE_TIMEOUTS: AtomicU64 = AtomicU64::new(0);
/// Procedures handed out by the DRR scheduler.
pub static SCHEDULER_DISPATCHED: AtomicU64 = AtomicU64::new(0);
/// Times the DRR scheduler moved a flow to the tail of the ring.
pub static SCHEDULER_ROTATIONS: AtomicU64 = AtomicU64::new(0);

/// Values of all counters at one moment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub in_flight_waits: u64,
    pub connection_data_waits: u64,
    pub data_budget_waits: u64,
    pub pending_waits: u64,
    pub connection_waits: u64,
    pub allocator_waits: u64,
    pub allocator_wait_ns: u64,
    pub write_timeouts: u64,
    pub scheduler_dispatched: u64,
    pub scheduler_rotations: u64,
}

/// Reads all counters.
pub fn snapshot() -> Snapshot {
    let load = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
    Snapshot {
        in_flight_waits: load(&IN_FLIGHT_WAITS),
        connection_data_waits: load(&CONNECTION_DATA_WAITS),
        data_budget_waits: load(&DATA_BUDGET_WAITS),
        pending_waits: load(&PENDING_WAITS),
        connection_waits: load(&CONNECTION_WAITS),
        allocator_waits: load(&ALLOCATOR_WAITS),
        allocator_wait_ns: load(&ALLOCATOR_WAIT_NS),
        write_timeouts: load(&WRITE_TIMEOUTS),
        scheduler_dispatched: load(&SCHEDULER_DISPATCHED),
        scheduler_rotations: load(&SCHEDULER_ROTATIONS),
    }
}

impl fmt::Display for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "in_flight_waits={} conn_data_waits={} data_budget_waits={} pending_waits={} \
             connection_waits={} allocator_waits={} allocator_wait_ms={:.1} write_timeouts={} \
             sched_dispatched={} sched_rotations={}",
            self.in_flight_waits,
            self.connection_data_waits,
            self.data_budget_waits,
            self.pending_waits,
            self.connection_waits,
            self.allocator_waits,
            self.allocator_wait_ns as f64 / 1e6,
            self.write_timeouts,
            self.scheduler_dispatched,
            self.scheduler_rotations,
        )
    }
}
