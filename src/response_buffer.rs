use std::io;

use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tracing::trace;

/// Constant to set last bit in Record Marking Standard
const LAST_FG_MASK: u32 = 1 << 31;
/// Max size of Record Marking Standard fragment
const MAX_RM_FRAGMENT_SIZE: usize = (1 << 31) - 1;

/// Represents a response buffer that minimizes data copying
pub struct ResponseBuffer {
    /// Internal buffer for writing data
    buffer: Vec<u8>,
    /// Indicates that the buffer contains data to send
    has_content: bool,
}

impl ResponseBuffer {
    /// Creates a new response buffer with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self { buffer: Vec::with_capacity(capacity), has_content: false }
    }

    /// Gets the internal buffer for writing
    pub fn get_mut_buffer(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    /// Marks the buffer as containing data to send
    pub fn mark_has_content(&mut self) {
        self.has_content = true;
    }

    /// Checks if the buffer contains data to send
    pub fn has_content(&self) -> bool {
        self.has_content
    }

    /// Takes the internal buffer, consuming the structure
    pub fn into_inner(self) -> Vec<u8> {
        self.buffer
    }

    /// Clears the buffer for reuse
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.has_content = false;
    }
    pub async fn write_fragment(
        &mut self,
        write_half: &mut WriteHalf<TcpStream>,
    ) -> io::Result<()> {
        // Maximum fragment size is 2^31 - 1 bytes

        let mut offset = 0;
        while offset < self.buffer.len() {
            // Calculate the size of this fragment
            let remaining = self.buffer.len() - offset;
            let fragment_size = std::cmp::min(remaining, MAX_RM_FRAGMENT_SIZE);

            // Determine if this is the last fragment
            let is_last = offset + fragment_size >= self.buffer.len();

            // Create the fragment header
            // The highest bit indicates if this is the last fragment
            let fragment_header =
                if is_last { fragment_size as u32 + LAST_FG_MASK } else { fragment_size as u32 };

            let header_buf = u32::to_be_bytes(fragment_header);
            write_half.write_all(&header_buf).await?;

            trace!("Writing fragment length:{}, last:{}", fragment_size, is_last);
            write_half.write_all(&self.buffer[offset..offset + fragment_size]).await?;

            offset += fragment_size;
        }

        Ok(())
    }
}
