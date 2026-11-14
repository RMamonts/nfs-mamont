//! Per-connection limits.

use std::num::NonZeroUsize;
use std::time::Duration;

/// Lower bound of the default [`crate::ServerContext::with_max_in_flight`].
///
/// A client needs about `throughput × server latency` requests in flight to keep the
/// server busy: 125 for 250 000 metadata operations per second at 0.5 ms.
pub const DEFAULT_MAX_IN_FLIGHT: NonZeroUsize = match NonZeroUsize::new(256) {
    Some(limit) => limit,
    None => unreachable!(),
};

/// In-flight requests per VFS worker the default limit allows at least.
///
/// While the pool executes `vfs_concurrency` requests of a connection, about as many
/// again are being parsed, queued or written back: a single client against 64 workers
/// and a 1 ms backend gets 44k operations per second with a limit of 64 and 58k with 128.
pub const IN_FLIGHT_PER_VFS_WORKER: NonZeroUsize = match NonZeroUsize::new(2) {
    Some(factor) => factor,
    None => unreachable!(),
};

/// Slots of the default [`crate::ServerContext::with_max_pending`] per VFS worker.
///
/// A few procedures queued behind every worker keep the pool busy; the rest wait in the
/// sockets of their connections, where TCP flow control holds the clients back.
///
/// Measured with 64 workers and GETATTR, for 2, 4 and 8 slots per worker. Next to 256
/// backlogged connections on a 1 ms backend a light one waits 6, 10 and 17 ms, against
/// 18 ms without the limit; next to 1024 of them 19, 21 and 25 ms against 67 ms. A server
/// short of CPU on a 200 us backend serves 7 to 15 % more calls with the limit, the more
/// the more slots there are. On a backend that never fills the pool the limit costs up
/// to 3 % of the throughput of a server on two cores, and nothing on four.
pub const PENDING_PER_VFS_WORKER: NonZeroUsize = match NonZeroUsize::new(4) {
    Some(factor) => factor,
    None => unreachable!(),
};

/// Default of [`crate::ServerContext::with_max_unverified_connections`], the same as the
/// Linux NFS server.
pub const DEFAULT_MAX_UNVERIFIED_CONNECTIONS: NonZeroUsize = match NonZeroUsize::new(64) {
    Some(limit) => limit,
    None => unreachable!(),
};

/// Default of [`crate::ServerContext::with_idle_timeout`]: a client that keeps a
/// connection without a single request for longer is most likely gone, and a live one
/// reconnects when it has a request to send.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(6 * 60);
