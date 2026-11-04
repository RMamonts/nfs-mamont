//! Defines [`Allocator`] interface used to bound allocation of buffers
//! for user data transmission inside NFS-Mamont implementation.

use std::future::Future;
use std::num::NonZeroUsize;

/// Abstract buffer type returned by [`Allocator`].
///
/// Implementations provide chunked read/write access to the allocated memory.
pub trait Buffer: Send + Sync {
    /// Returns an iterator over read-only byte chunks of this buffer.
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_;

    /// Returns an iterator over mutable byte chunks of this buffer.
    fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_;

    /// Returns the total number of bytes in the buffer.
    fn len(&self) -> usize;

    /// Returns `true` if the buffer is empty.
    fn is_empty(&self) -> bool;

    /// Creates an empty, zero-length buffer with no backing memory.
    fn empty() -> Self
    where
        Self: Sized;
}

/// Allocates buffers for user data transmission inside NFS-Mamont implementation.
pub trait Allocator {
    /// Type of buffer returned by this allocator.
    type Buffer: Buffer;

    /// Returns a buffer of at least `size` bytes.
    ///
    /// Waits until enough memory is available, so the returned future may stay
    /// pending while the pool is exhausted.
    ///
    /// # Parameters
    ///
    /// - `size` --- minimum size of the returned buffer in bytes.
    ///
    /// # Returns
    ///
    /// Returns [`None`] if `size` is greater than [`Allocator::capacity`],
    /// since such a request can never be satisfied.
    fn allocate(&self, size: NonZeroUsize) -> impl Future<Output = Option<Self::Buffer>> + Send;

    /// Returns the largest size [`Allocator::allocate`] can ever satisfy.
    ///
    /// Callers that are free to shorten their request (for example NFSv3 `READ`,
    /// where a short read is legal) should clamp to this value instead of
    /// failing on an oversized request.
    fn capacity(&self) -> NonZeroUsize;
}
