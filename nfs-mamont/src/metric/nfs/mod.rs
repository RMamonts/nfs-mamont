//! Metrics of the NFS request pipeline.

#[cfg(feature = "metrics")]
pub(in crate::metric) mod consts;
#[cfg(feature = "metrics")]
pub(in crate::metric) mod io;
#[cfg(feature = "metrics")]
mod nfs_metrics;
#[cfg(feature = "metrics")]
pub use nfs_metrics::*;
#[cfg(all(test, feature = "metrics"))]
mod io_test;
#[cfg(all(test, feature = "metrics"))]
mod test;

#[cfg(not(feature = "metrics"))]
mod noop;
#[cfg(not(feature = "metrics"))]
pub use noop::*;
