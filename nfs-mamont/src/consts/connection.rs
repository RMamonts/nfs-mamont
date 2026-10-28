//! Per-connection limits.

use std::num::NonZeroUsize;

/// Default number of requests of one connection that may be read from the socket but
/// not yet answered, see [`crate::ServerContext::with_max_in_flight`].
///
/// A client needs about `throughput × server latency` requests in flight to keep the
/// server busy: 125 for 250 000 metadata operations per second at 0.5 ms, a handful for
/// 1 MiB reads over 10 GbE. The limit bounds the memory held by one connection to
/// `DEFAULT_MAX_IN_FLIGHT × reply size`.
pub const DEFAULT_MAX_IN_FLIGHT: NonZeroUsize = match NonZeroUsize::new(256) {
    Some(limit) => limit,
    None => unreachable!(),
};
