//! Server-wide budgets shared by every connection, configured through
//! [`crate::ServerContext`].
//!
//! Every budget is tied to one resource and one point where it is given back:
//!
//! | Budget | Unit | Taken | Given back |
//! |---|---|---|---|
//! | `max_connections` | connection | before `accept` | both tasks of the connection are gone |
//! | `max_pending` | request | after a request is parsed, before it is queued | the VFS/MOUNT/NLM task finished it |
//! | `data_budget` | allocator granularity | before a READ/WRITE buffer is allocated | the reply is written |
//!
//! The per-connection limits (`max_in_flight`, `max_data_per_connection`) are given
//! back when the reply is written, see [`crate::task::connection`].

use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

/// How the VFS pool picks the next procedure to execute, see
/// [`crate::ServerContext::with_scheduling`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheduling {
    /// One shared queue served in arrival order.
    Fifo,
    /// A queue per connection served by deficit round robin.
    ///
    /// Every round, each connection with queued procedures may spend `quantum` cost
    /// units; a procedure costs one unit plus one per started
    /// [`crate::consts::connection::COST_UNIT_BYTES`] of READ or WRITE data. A
    /// connection whose queue runs empty loses its unspent quantum, so idle connections
    /// bank no credit. Connections that compete for the pool get equal cost shares,
    /// a connection that asks for less gets all it asks for, and a connection alone
    /// gets every worker.
    Drr {
        /// Cost units one connection may spend per round.
        quantum: NonZeroU32,
    },
}

/// Global budget of allocator memory, counted in units of the allocator granularity.
pub(crate) struct DataBudget {
    /// One permit per unit of [`Limits::granularity`] bytes.
    pub semaphore: Arc<Semaphore>,
    /// Number of units the budget was created with.
    pub units: usize,
}

/// Limits of the server, see the module docs.
pub(crate) struct Limits {
    /// Requests one connection may have read but not yet answered.
    pub max_in_flight: NonZeroUsize,
    /// Bytes of allocator memory one connection may hold, `None` for no limit.
    pub max_data_per_connection: Option<u64>,
    /// Open connections, `None` for no limit.
    pub connections: Option<Arc<Semaphore>>,
    /// Requests parsed but not yet finished by a global task, `None` for no limit.
    pub pending: Option<Arc<Semaphore>>,
    /// Allocator memory in flight, `None` for no limit.
    pub data: Option<DataBudget>,
    /// Size the allocator rounds requests up to, see [`crate::Allocator::granularity`].
    pub granularity: NonZeroUsize,
    /// How long the write task waits for the socket to take a reply, `None` for ever.
    pub write_timeout: Option<Duration>,
}

impl Limits {
    /// Limits with only the per-connection in-flight budget set.
    pub fn new(max_in_flight: NonZeroUsize, granularity: NonZeroUsize) -> Self {
        Self {
            max_in_flight,
            max_data_per_connection: None,
            connections: None,
            pending: None,
            data: None,
            granularity,
            write_timeout: None,
        }
    }
}
