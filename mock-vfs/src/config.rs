//! Configuration of the mock backend.

use std::num::NonZeroU64;
use std::time::Duration;

/// Default size reported for every regular file: 1 GiB.
pub const DEFAULT_FILE_SIZE: u64 = 1 << 30;
/// Default number of regular files listed in every directory.
pub const DEFAULT_FILES_PER_DIR: usize = 128;
/// Default number of subdirectories listed in every directory.
pub const DEFAULT_DIRS_PER_DIR: usize = 8;
/// Default maximum READ/WRITE size advertised through `FSINFO`: 1 MiB.
pub const DEFAULT_MAX_IO: u32 = 1 << 20;

/// Response time of the simulated backend.
///
/// A request transferring `bytes` bytes that is issued at `now` completes at
///
/// ```text
/// max(now, device_free_at) + bytes / bandwidth + base ± jitter
/// ```
///
/// `device_free_at` is shared by all requests, so `bandwidth` caps the aggregate
/// throughput of READ and WRITE no matter how many of them run in parallel, while
/// `base` (the per-request latency) is paid by every request independently. Metadata
/// procedures transfer no bytes and therefore only pay `base ± jitter`.
///
/// With `N` VFS workers (the `vfs_concurrency` of [`nfs_mamont::ServerContext::new`])
/// and a latency `L`, the server cannot complete more than `N / L` procedures per
/// second: `N` plays the role of the queue depth of the simulated backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Latency {
    /// Fixed delay paid by every procedure.
    pub base: Duration,
    /// Half-width of the uniform jitter added to [`Self::base`]: the actual delay is
    /// drawn from `[base - jitter, base + jitter]` and never goes below zero.
    pub jitter: Duration,
    /// Throughput of the simulated storage device in bytes per second, shared by all
    /// READ and WRITE requests. [`None`] means unlimited.
    pub bandwidth: Option<NonZeroU64>,
}

impl Latency {
    /// No delay at all: every procedure completes as soon as it is executed.
    pub const INSTANT: Latency =
        Latency { base: Duration::ZERO, jitter: Duration::ZERO, bandwidth: None };

    /// Fixed per-request latency without jitter or bandwidth limit.
    pub const fn fixed(base: Duration) -> Latency {
        Latency { base, jitter: Duration::ZERO, bandwidth: None }
    }

    /// Returns `true` when the configuration adds no delay.
    pub fn is_instant(&self) -> bool {
        self.base.is_zero() && self.jitter.is_zero() && self.bandwidth.is_none()
    }
}

/// Configuration of [`crate::mock::MockVfs`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockVfsConfig {
    /// Response time of the backend.
    pub latency: Latency,
    /// Size reported for every regular file; READ returns EOF past it.
    pub file_size: u64,
    /// Number of regular files (`file_0`, `file_1`, ...) listed in every directory.
    pub files_per_dir: usize,
    /// Number of subdirectories (`dir_0`, `dir_1`, ...) listed in every directory.
    pub dirs_per_dir: usize,
    /// Maximum READ/WRITE transfer size advertised through `FSINFO`.
    ///
    /// Must not exceed the capacity of the server allocator (buffer size times buffer
    /// count), otherwise clients may issue requests the server cannot allocate for.
    pub max_io: u32,
    /// Byte pattern READ fills its buffer with. [`None`] leaves the buffer untouched,
    /// which is the cheapest option and the right one for measuring the server.
    pub read_fill: Option<u8>,
}

impl Default for MockVfsConfig {
    fn default() -> Self {
        Self {
            latency: Latency::INSTANT,
            file_size: DEFAULT_FILE_SIZE,
            files_per_dir: DEFAULT_FILES_PER_DIR,
            dirs_per_dir: DEFAULT_DIRS_PER_DIR,
            max_io: DEFAULT_MAX_IO,
            read_fill: None,
        }
    }
}
