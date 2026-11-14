//! Limits that keep clients from taking more of the server than their share.
//!
//! All of them are set through [`ServerContext`] and kept in [`Limits`]. They are
//! enforced by one object per scope:
//!
//! - [`Admission`] lets new connections in, in the accept loop;
//! - [`ConnectionLimits`] decide whether the read task of a connection may read the next
//!   request and whether the connection has to close;
//! - the write task of a connection enforces its write timeout, see
//!   [`crate::task::connection`];
//! - the VFS pool's [`drr`] channel holds the procedures of all connections.
//!
//! This is their map, in the order a request meets them: for each limit its default,
//! where it is enforced and what happens at it.
//!
//! # Connections
//!
//! - [`Limits::max_connections`], off. [`Admission::accept`] takes a slot for every new
//!   connection, freed once the write task of the connection is gone. At the limit the
//!   oldest connection on probation is closed; without one, `accept` waits.
//! - [`Limits::max_unverified_connections`], [`DEFAULT_MAX_UNVERIFIED_CONNECTIONS`].
//!   [`Admission::accept`] puts every new connection on [`Probation`];
//!   [`ConnectionLimits::verified`] takes it off with the first valid call. At the limit
//!   the oldest connection on probation is closed.
//! - [`Limits::idle_timeout`], [`DEFAULT_IDLE_TIMEOUT`]. [`ConnectionLimits::expired`],
//!   polled by the read task while it waits for a request. The connection is closed.
//!
//! # Requests of one connection
//!
//! - [`Limits::max_in_flight`], the larger of [`DEFAULT_MAX_IN_FLIGHT`] and
//!   [`IN_FLIGHT_PER_VFS_WORKER`] × `vfs_concurrency`. [`ConnectionLimits::acquire`]
//!   takes a slot before a request is read; the write task frees it once the reply is
//!   written. At the limit the socket stays unread and TCP flow control holds the client
//!   back.
//! - [`Limits::max_data_per_connection`], [`Allocator::pool_size`] divided by
//!   [`CONNECTIONS_PER_POOL`], off if the allocator tells no pool size.
//!   [`ConnectionLimits::charge`] counts READ and WRITE data by [`Buffer::footprint`].
//!   At the limit the socket stays unread.
//! - [`Limits::write_timeout`], [`DEFAULT_WRITE_TIMEOUT`]. The write task, for every
//!   reply. The connection is closed.
//!
//! # Shared by all connections
//!
//! - [`Limits::max_pending`], [`PENDING_PER_VFS_WORKER`] × `vfs_concurrency`: the
//!   capacity of the [`drr`] channel. [`FlowSender::send`] takes a slot before a
//!   procedure is queued for the VFS pool, freed once its reply is queued. At the limit
//!   the read tasks wait, the one that waits longest first.
//! - [`Limits::vfs_concurrency`], no default. [`VfsManager`] runs that many workers, each
//!   executing one procedure at a time. At the limit procedures wait in their queues.
//! - The allocator pool, [`Allocator::capacity`], set by the allocator.
//!   [`Allocator::allocate`], called by the parser in the read task for READ and WRITE
//!   data. At the limit the read task waits for memory.
//!
//! The [`drr`] channel serves the connections in deficit round robin; that shares the
//! workers fairly but limits nothing.
//!
//! # What bounds what
//!
//! - The result channel of a connection holds at most `max_in_flight` replies, so the
//!   global tasks never wait for a slow client.
//! - The VFS pool holds at most `max_pending` NFS procedures, however many connections
//!   there are; the MOUNT and NLM queues hold at most `max_in_flight` requests per open
//!   connection.
//! - READ and WRITE data of all connections never exceeds the allocator pool;
//!   `max_data_per_connection` keeps one connection from taking all of it, and
//!   `write_timeout` frees what a client that stopped reading holds.
//!
//! [`ServerContext`]: crate::ServerContext
//! [`Admission`]: admission::Admission
//! [`Admission::accept`]: admission::Admission::accept
//! [`Probation`]: probation::Probation
//! [`ConnectionLimits`]: connection::ConnectionLimits
//! [`ConnectionLimits::verified`]: connection::ConnectionLimits::verified
//! [`ConnectionLimits::expired`]: connection::ConnectionLimits::expired
//! [`ConnectionLimits::acquire`]: connection::ConnectionLimits::acquire
//! [`ConnectionLimits::charge`]: connection::ConnectionLimits::charge
//! [`drr`]: crate::task::global::vfs::drr
//! [`FlowSender::send`]: crate::task::global::vfs::drr::FlowSender::send
//! [`VfsManager`]: crate::task::global::vfs::VfsManager
//! [`Buffer::footprint`]: crate::Buffer::footprint
//! [`Allocator::capacity`]: crate::Allocator::capacity
//! [`Allocator::pool_size`]: crate::Allocator::pool_size
//! [`Allocator::allocate`]: crate::Allocator::allocate

pub mod admission;
pub mod budget;
pub mod connection;
pub mod probation;

use std::num::NonZeroUsize;
use std::time::Duration;

use crate::consts::connection::{
    CONNECTIONS_PER_POOL, DEFAULT_IDLE_TIMEOUT, DEFAULT_MAX_IN_FLIGHT,
    DEFAULT_MAX_UNVERIFIED_CONNECTIONS, DEFAULT_WRITE_TIMEOUT, IN_FLIGHT_PER_VFS_WORKER,
    PENDING_PER_VFS_WORKER,
};

/// Limits of a server, set through [`crate::ServerContext::new`] and its `with_*` methods
/// and read with [`crate::ServerContext::limits`].
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
    /// NFS procedures that may wait for or run in the VFS pool at once, see
    /// [`crate::ServerContext::with_max_pending`].
    pub max_pending: NonZeroUsize,
    /// NFS procedures executed against the backends at once, see
    /// [`crate::ServerContext::new`].
    pub vfs_concurrency: NonZeroUsize,
}

/// Returns the default limits for a VFS pool of `vfs_concurrency` workers and an
/// allocator pool of `pool_size` bytes, if the allocator tells one, see the
/// [module docs](self).
pub fn defaults(vfs_concurrency: NonZeroUsize, pool_size: Option<NonZeroUsize>) -> Limits {
    Limits {
        max_connections: None,
        max_unverified_connections: DEFAULT_MAX_UNVERIFIED_CONNECTIONS,
        idle_timeout: Some(DEFAULT_IDLE_TIMEOUT),
        max_in_flight: default_max_in_flight(vfs_concurrency),
        max_data_per_connection: default_max_data_per_connection(pool_size),
        write_timeout: Some(DEFAULT_WRITE_TIMEOUT),
        max_pending: default_max_pending(vfs_concurrency),
        vfs_concurrency,
    }
}

/// Returns the default in-flight limit for a pool of `vfs_concurrency` workers.
fn default_max_in_flight(vfs_concurrency: NonZeroUsize) -> NonZeroUsize {
    DEFAULT_MAX_IN_FLIGHT.max(vfs_concurrency.saturating_mul(IN_FLIGHT_PER_VFS_WORKER))
}

/// Returns the default data limit of one connection: its share of an allocator pool of
/// `pool_size` bytes, none without a pool size or with a pool too small to share.
fn default_max_data_per_connection(pool_size: Option<NonZeroUsize>) -> Option<NonZeroUsize> {
    NonZeroUsize::new(pool_size?.get() / CONNECTIONS_PER_POOL)
}

/// Returns the default limit of a pool of `vfs_concurrency` workers.
fn default_max_pending(vfs_concurrency: NonZeroUsize) -> NonZeroUsize {
    vfs_concurrency.saturating_mul(PENDING_PER_VFS_WORKER)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::{default_max_data_per_connection, default_max_in_flight, default_max_pending};
    use crate::consts::connection::DEFAULT_MAX_IN_FLIGHT;

    #[test]
    fn the_default_limit_grows_with_the_pool() {
        let limit = |workers| default_max_in_flight(NonZeroUsize::new(workers).unwrap()).get();

        assert_eq!(limit(1), DEFAULT_MAX_IN_FLIGHT.get());
        assert_eq!(limit(128), DEFAULT_MAX_IN_FLIGHT.get());
        assert_eq!(limit(129), 258);
        assert_eq!(limit(usize::MAX), usize::MAX);
    }

    #[test]
    fn the_default_pool_limit_grows_with_the_pool() {
        let limit = |workers| default_max_pending(NonZeroUsize::new(workers).unwrap()).get();

        assert_eq!(limit(1), 4);
        assert_eq!(limit(64), 256);
        assert_eq!(limit(usize::MAX), usize::MAX);
    }

    #[test]
    fn the_default_data_limit_is_a_share_of_the_pool() {
        let limit = |pool| default_max_data_per_connection(NonZeroUsize::new(pool));

        assert_eq!(limit(0), None, "no pool size, no limit");
        assert_eq!(limit(3), None, "a pool too small to share");
        assert_eq!(limit(4), NonZeroUsize::new(1));
        assert_eq!(limit(64 << 20), NonZeroUsize::new(16 << 20));
        assert_eq!(limit(usize::MAX), NonZeroUsize::new(usize::MAX / 4));
    }
}
