//! Page-aligned, lazily-zeroed memory region obtained directly from `mmap`.
//!
//! `std::alloc::alloc_zeroed` with an alignment above 16 falls back to
//! `posix_memalign` + `memset`, which touches (and therefore commits) every page up
//! front. `mmap` gives page alignment *and* lazy zero pages.

use std::ptr;

const HUGE: usize = 2 << 20;

pub struct Arena {
    base: *mut u8,
    len: usize,
    /// Deliberate misalignment of `base` (control experiments only).
    offset: usize,
}

// The arena is plain memory; synchronisation of its contents is the job of the pool.
unsafe impl Send for Arena {}
unsafe impl Sync for Arena {}

impl Arena {
    /// Maps `len` bytes. With `huge_pages` the region is 2 MiB aligned and marked
    /// `MADV_HUGEPAGE` so THP can back it even when THP is in `madvise` mode.
    pub fn new(len: usize, huge_pages: bool) -> Self {
        Self::with_offset(len, huge_pages, 0)
    }

    /// Like [`Self::new`], but `base` is shifted by `offset` bytes from the page
    /// boundary (upstream's buffers start 16 bytes past a page boundary).
    pub fn with_offset(len: usize, huge_pages: bool, offset: usize) -> Self {
        assert!(len > 0 && offset < 4096);
        let len = len + offset;
        let align = if huge_pages { HUGE } else { 4096 };
        let map_len = len + align;
        // SAFETY: anonymous private mapping, arguments are valid.
        let raw = unsafe {
            libc::mmap(
                ptr::null_mut(),
                map_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert!(raw != libc::MAP_FAILED, "mmap failed: {}", std::io::Error::last_os_error());
        let raw = raw as usize;
        let start = (raw + align - 1) & !(align - 1);
        let end = start + len;
        // SAFETY: unmapping the unused head/tail of our own mapping.
        unsafe {
            if start > raw {
                libc::munmap(raw as *mut _, start - raw);
            }
            let map_end = raw + map_len;
            if map_end > end {
                libc::munmap(end as *mut _, map_end - end);
            }
            if huge_pages {
                libc::madvise(start as *mut _, len, libc::MADV_HUGEPAGE);
            }
        }
        Self { base: (start + offset) as *mut u8, len: len - offset, offset }
    }

    #[inline]
    pub fn base(&self) -> *mut u8 {
        self.base
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        // SAFETY: the region was mapped in `new` and is not referenced any more.
        unsafe {
            libc::munmap(self.base.sub(self.offset) as *mut _, self.len + self.offset);
        }
    }
}
