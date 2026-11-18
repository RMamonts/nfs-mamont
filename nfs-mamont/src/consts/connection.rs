//! Per-connection limits.

use std::num::{NonZeroU32, NonZeroUsize};
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

/// Cost units a connection may spend per turn of the VFS pool, the quantum of its
/// [`drr_channel`].
///
/// A light connection waits for one quantum of every backlogged one, so a small quantum
/// isolates connections better; but the procedures of one connection then no longer
/// follow each other, and the pool loses the batching of replies per connection.
///
/// Measured with the pool's default capacity of 256, for a quantum of 8, 4, 2 and 1. On a
/// 1 ms backend, next to 16 connections with 64 GETATTRs in flight each, a light
/// connection waits 3.6, 2.6, 2.1 and 1.8 ms; next to 64 connections with 16 each, 8.1,
/// 6.2, 4.2 and 3.2 ms. With as many connections as the capacity or more, every
/// connection has about one procedure queued and the wait is close to 10 ms whatever the
/// quantum. A server short of CPU on a 200 us backend loses 0, 0, 1 and 4 % of its
/// throughput.
pub const VFS_QUANTUM: NonZeroU32 = match NonZeroU32::new(4) {
    Some(quantum) => quantum,
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

/// How many connections it takes to hold the whole allocator pool: the default
/// [`crate::ServerContext::with_max_data_per_connection`] is the pool divided by it.
///
/// A client that stops reading its READ replies keeps their buffers until the write
/// timeout closes it; with this share, three such clients leave room for the others.
/// The limit costs about 1 % of the throughput of 4 KiB WRITEs and nothing of GETATTR
/// and READ.
pub const CONNECTIONS_PER_POOL: NonZeroUsize = match NonZeroUsize::new(4) {
    Some(connections) => connections,
    None => unreachable!(),
};

/// Default of [`crate::ServerContext::with_write_timeout`], the time after which a Linux
/// NFS client gives a call over TCP up as lost: a client that has not taken a reply for
/// that long no longer waits for it.
///
/// The timeout is armed for every reply, which costs a server short of CPU 2 to 3 % of
/// its GETATTR and 4 KiB READ throughput.
pub const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(60);

/// Default of [`crate::ServerContext::with_idle_timeout`]: a client that keeps a
/// connection without a single request for longer is most likely gone, and a live one
/// reconnects when it has a request to send.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(6 * 60);
