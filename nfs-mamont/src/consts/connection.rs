//! Per-connection limits.

use std::num::NonZeroUsize;

/// Lower bound of the default number of requests of one connection that may be read
/// from the socket but not yet answered, see
/// [`crate::ServerContext::with_max_in_flight`].
///
/// A client needs about `throughput × server latency` requests in flight to keep the
/// server busy: 125 for 250 000 metadata operations per second at 0.5 ms, a handful for
/// 1 MiB reads over 10 GbE. The limit bounds the memory held by one connection to
/// `DEFAULT_MAX_IN_FLIGHT × reply size`.
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
