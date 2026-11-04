//! Per-connection and server-wide limits.

use std::num::{NonZeroU32, NonZeroUsize};

/// Lower bound of the default number of requests of one connection that may be read
/// from the socket but not yet answered, see
/// [`crate::ServerContext::with_max_in_flight`].
///
/// A client needs about `throughput × server latency` requests in flight to keep the
/// server busy: 125 for 250 000 metadata operations per second at 0.5 ms, a handful for
/// 1 MiB reads over 10 GbE. The limit bounds the replies queued by one connection, not
/// the allocator buffers its READ and WRITE requests hold: 256 reads of 1 MiB take more
/// than a pool of 64 buffers of 1 MiB, see [`crate::ServerContext::with_max_in_flight`].
pub const DEFAULT_MAX_IN_FLIGHT: NonZeroUsize = match NonZeroUsize::new(256) {
    Some(limit) => limit,
    None => unreachable!(),
};

/// Requests per VFS worker the default in-flight limit allows at least: the default is
/// the larger of [`DEFAULT_MAX_IN_FLIGHT`] and `IN_FLIGHT_PER_VFS_WORKER ×
/// vfs_concurrency`, see [`crate::ServerContext::new`].
///
/// While the pool executes `vfs_concurrency` requests of a connection, about as many
/// again are being parsed, wait in the queue or travel back through the write task.
/// With a limit equal to the pool size one connection keeps it busy only part of the
/// time: a single client against 64 workers and a 1 ms backend gets 44k operations per
/// second with a limit of 64, 58k with 128 and 58k with 256.
pub const IN_FLIGHT_PER_VFS_WORKER: NonZeroUsize = match NonZeroUsize::new(2) {
    Some(factor) => factor,
    None => unreachable!(),
};

/// Pending requests per VFS worker the default global pending limit allows, see
/// [`crate::default_max_pending`]: `vfs_concurrency` requests execute while as many
/// again are queued, parsed or on their way, so a few per worker keep the pool busy
/// and more only adds queueing delay.
pub const PENDING_PER_VFS_WORKER: NonZeroUsize = match NonZeroUsize::new(4) {
    Some(factor) => factor,
    None => unreachable!(),
};

/// Bytes of READ or WRITE data per cost unit of the DRR scheduler, see
/// [`crate::Scheduling::Drr`]: a procedure costs one unit plus one per started
/// `COST_UNIT_BYTES`, so a GETATTR costs 1 and a 1 MiB READ 17.
pub const COST_UNIT_BYTES: usize = 64 * 1024;

/// Default quantum of the DRR scheduler in cost units: per round a connection with
/// metadata procedures gets up to 8 of them and a connection streaming 1 MiB reads
/// one read per three rounds, see [`crate::Scheduling::Drr`].
///
/// A small quantum keeps the wait of a light connection short: on a 1 ms backend with
/// 64 workers a depth-1 client next to a depth-2048 one sees a p50 of 1.8 ms with a
/// quantum of 32 and 1.4 ms with 8, at no measurable cost in throughput
/// (`research/qos-limits`, E7b).
pub const DEFAULT_DRR_QUANTUM: NonZeroU32 = match NonZeroU32::new(8) {
    Some(quantum) => quantum,
    None => unreachable!(),
};
