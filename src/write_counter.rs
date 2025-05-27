//! Write Counter module provides a wrapper for Writer implementations that counts
//! bytes written during write operations.
//!
//! This module is particularly useful when implementing size-limited responses in NFS
//! operations, such as READDIR and READDIRPLUS, where responses need to be truncated
//! to fit within a specific byte limit.

#![allow(dead_code)]
use std::io::Write;

/// A wrapper around a Writer that counts the number of bytes written
///
/// This struct decorates any type implementing the Write trait and keeps track of
/// how many bytes have been successfully written. This is particularly useful in NFS
/// protocol operations where responses need to be limited to a specific size.
pub struct WriteCounter<W> {
    /// The wrapped writer instance
    inner: W,
    /// Count of bytes successfully written so far
    count: usize,
}

impl<W> WriteCounter<W>
where
    W: Write,
{
    /// Creates a new WriteCounter wrapping the provided writer
    ///
    /// # Arguments
    ///
    /// * `inner` - The writer implementation to wrap
    ///
    /// # Returns
    ///
    /// A new WriteCounter with a zero byte count
    pub fn new(inner: W) -> Self {
        WriteCounter { inner, count: 0 }
    }

    /// Consumes the WriteCounter and returns the wrapped writer
    ///
    /// # Returns
    ///
    /// The original writer that was wrapped
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Returns the current count of bytes written
    ///
    /// # Returns
    ///
    /// The total number of bytes successfully written so far
    pub fn bytes_written(&self) -> usize {
        self.count
    }
}

impl<W> Write for WriteCounter<W>
where
    W: Write,
{
    /// Writes a buffer to the wrapped writer and counts the bytes written
    ///
    /// This method delegates to the inner writer and increments the byte counter
    /// by the number of bytes actually written.
    ///
    /// # Arguments
    ///
    /// * `buf` - The buffer to write
    ///
    /// # Returns
    ///
    /// A Result containing the number of bytes written or an error
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let res = self.inner.write(buf);
        if let Ok(size) = res {
            self.count += size
        }
        res
    }

    /// Flushes the wrapped writer
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
