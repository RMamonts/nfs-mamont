//! Limits that keep clients from taking more of the server than their share.
//!
//! All of them are set through [`ServerContext`] and kept in [`Limits`]. This is the map
//! of what each one bounds and where it is enforced, in the order a request meets them.
//! The read and write tasks of a connection are described in [`crate::task::connection`].
//!
//! Each entry gives the default, where the limit is enforced and what happens at it.
//!
//! # Connections
//!
//! - [`Limits::max_connections`], off. [`crate::accept`] takes a slot for every new
//!   connection, freed once the write task of the connection is gone. At the limit the
//!   oldest connection on probation is closed; without one, `accept` waits.
//! - [`Limits::max_unverified_connections`], [`DEFAULT_MAX_UNVERIFIED_CONNECTIONS`].
//!   [`Probation`] admits every new connection; the read task leaves it with the first
//!   valid call. At the limit the oldest connection on probation is closed.
//! - [`Limits::idle_timeout`], [`DEFAULT_IDLE_TIMEOUT`]. [`Budget::idle`], polled by the
//!   read task while it waits for a request. The connection is closed.
//!
//! # Requests of one connection
//!
//! - [`Limits::max_in_flight`], the larger of [`DEFAULT_MAX_IN_FLIGHT`] and
//!   [`IN_FLIGHT_PER_VFS_WORKER`] × `vfs_concurrency`. [`Budget::acquire`] takes a slot
//!   before a request is read; the write task frees it once the reply is written. At the
//!   limit the socket stays unread and TCP flow control holds the client back.
//! - [`Limits::max_data_per_connection`], off. [`Budget::charge`] counts READ and WRITE
//!   data by [`Buffer::footprint`]. At the limit the socket stays unread.
//! - [`Limits::write_timeout`], off. The write task, for every reply. The connection is
//!   closed.
//!
//! # Shared by all connections
//!
//! - [`Limits::max_pending`], off. [`VfsSender::send`] takes a slot before a procedure is
//!   queued for the VFS pool, freed once its reply is queued. At the limit the read task
//!   waits.
//! - The allocator pool, [`Allocator::capacity`], set by the allocator.
//!   [`Allocator::allocate`], called by the parser in the read task for READ and WRITE
//!   data. At the limit the read task waits for memory.
//!
//! The VFS pool serves the connections in deficit round robin, see
//! [`crate::task::global::vfs::scheduler`]; that shares the workers fairly but limits
//! nothing.
//!
//! # What bounds what
//!
//! - The result channel of a connection holds at most `max_in_flight` replies, so the
//!   global tasks never wait for a slow client.
//! - The global queues hold at most `max_in_flight` requests per open connection, and at
//!   most `max_pending` NFS procedures when it is set.
//! - READ and WRITE data of all connections never exceeds the allocator pool;
//!   `max_data_per_connection` keeps one connection from taking all of it, and
//!   `write_timeout` frees what a client that stopped reading holds.
//!
//! [`ServerContext`]: crate::ServerContext
//! [`Probation`]: crate::task::connection::probation::Probation
//! [`Budget::idle`]: crate::task::connection::limit::Budget::idle
//! [`Budget::acquire`]: crate::task::connection::limit::Budget::acquire
//! [`Budget::charge`]: crate::task::connection::limit::Budget::charge
//! [`VfsSender::send`]: crate::task::global::vfs::VfsSender::send
//! [`Buffer::footprint`]: crate::Buffer::footprint
//! [`Allocator::capacity`]: crate::Allocator::capacity
//! [`Allocator::allocate`]: crate::Allocator::allocate

use std::num::NonZeroUsize;
use std::time::Duration;

use crate::consts::connection::{
    DEFAULT_IDLE_TIMEOUT, DEFAULT_MAX_IN_FLIGHT, DEFAULT_MAX_UNVERIFIED_CONNECTIONS,
    IN_FLIGHT_PER_VFS_WORKER,
};

/// Limits of a server, set through the `with_*` methods of [`crate::ServerContext`] and
/// read with [`crate::ServerContext::limits`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Limits {
    /// Open connections, `None` for no limit, see
    /// [`crate::ServerContext::with_max_connections`].
    pub max_connections: Option<NonZeroUsize>,
    /// Connections that have not sent a valid call yet, see
    /// [`crate::ServerContext::with_max_unverified_connections`].
    pub max_unverified_connections: NonZeroUsize,
    /// How long a connection may stay without requests in flight, `None` for ever, see
    /// [`crate::ServerContext::with_idle_timeout`].
    pub idle_timeout: Option<Duration>,
    /// Requests one connection may have read but not yet answered, see
    /// [`crate::ServerContext::with_max_in_flight`].
    pub max_in_flight: NonZeroUsize,
    /// Bytes of READ and WRITE data one connection may have in flight, `None` for no
    /// limit, see [`crate::ServerContext::with_max_data_per_connection`].
    pub max_data_per_connection: Option<NonZeroUsize>,
    /// How long a reply may wait for the socket before the connection is closed, `None`
    /// for ever, see [`crate::ServerContext::with_write_timeout`].
    pub write_timeout: Option<Duration>,
    /// NFS procedures that may wait for or run in the VFS pool at once, `None` for no
    /// limit, see [`crate::ServerContext::with_max_pending`].
    pub max_pending: Option<NonZeroUsize>,
}

/// Returns the default limits for a VFS pool of `vfs_concurrency` workers, see the
/// [module docs](self).
pub fn defaults(vfs_concurrency: NonZeroUsize) -> Limits {
    Limits {
        max_connections: None,
        max_unverified_connections: DEFAULT_MAX_UNVERIFIED_CONNECTIONS,
        idle_timeout: Some(DEFAULT_IDLE_TIMEOUT),
        max_in_flight: default_max_in_flight(vfs_concurrency),
        max_data_per_connection: None,
        write_timeout: None,
        max_pending: None,
    }
}

/// Returns the default in-flight limit for a pool of `vfs_concurrency` workers.
fn default_max_in_flight(vfs_concurrency: NonZeroUsize) -> NonZeroUsize {
    DEFAULT_MAX_IN_FLIGHT.max(vfs_concurrency.saturating_mul(IN_FLIGHT_PER_VFS_WORKER))
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::default_max_in_flight;
    use crate::consts::connection::DEFAULT_MAX_IN_FLIGHT;

    #[test]
    fn the_default_limit_grows_with_the_pool() {
        let limit = |workers| default_max_in_flight(NonZeroUsize::new(workers).unwrap()).get();

        assert_eq!(limit(1), DEFAULT_MAX_IN_FLIGHT.get());
        assert_eq!(limit(128), DEFAULT_MAX_IN_FLIGHT.get());
        assert_eq!(limit(129), 258);
        assert_eq!(limit(usize::MAX), usize::MAX);
    }
}
