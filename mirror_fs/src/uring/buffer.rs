use std::io;

use io_uring::IoUring;

pub const MAX_BATCH: usize = 64;
pub const BATCH_WAIT: std::time::Duration = std::time::Duration::from_micros(50);
pub const DEFAULT_FIXED_BUFFER_COUNT: usize = 128;
pub const DEFAULT_FIXED_BUFFER_LEN: usize = 64 * 1024;

pub struct FixedBufferPool {
    buffers: Vec<Vec<u8>>,
    free: Vec<usize>,
    len: usize,
}

impl FixedBufferPool {
    pub fn new(ring: &mut IoUring, count: usize, len: usize) -> io::Result<Self> {
        if count == 0 || len == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid fixed buffer pool"));
        }

        let mut buffers = Vec::with_capacity(count);
        let mut iovecs = Vec::with_capacity(count);
        for _ in 0..count {
            let mut buffer = vec![0u8; len];
            let iovec =
                libc::iovec { iov_base: buffer.as_mut_ptr().cast(), iov_len: buffer.len() };
            buffers.push(buffer);
            iovecs.push(iovec);
        }

        unsafe {
            ring.submitter().register_buffers(&iovecs)?;
        }

        let free = (0..count).rev().collect();
        Ok(Self { buffers, free, len })
    }

    pub fn take(&mut self) -> Option<usize> {
        self.free.pop()
    }

    pub fn release(&mut self, index: usize) {
        self.free.push(index);
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(crate) fn buffer_mut(&mut self, index: usize) -> &mut [u8] {
        &mut self.buffers[index]
    }

    pub(crate) fn buffer(&self, index: usize) -> &[u8] {
        &self.buffers[index]
    }

    pub(crate) fn buffer_mut_ref(&mut self, index: usize) -> *mut u8 {
        self.buffers[index].as_mut_ptr()
    }
}
