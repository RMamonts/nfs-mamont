#![allow(dead_code)]

use std::alloc::{self, Layout, LayoutError};
use std::ptr::NonNull;

use tokio::sync::mpsc;

// TODO(i.erin) add miri tests
// TODO(i.erin) more tests!!!
// TODO(i.erin) iterator over buffer chain?

/// Intrusive linked list of buffers.
#[repr(C)]
struct BufferLayout {
    // Pointer to the next entry of the intrusive linked list.
    next: Option<Buffer>,

    // Allocation layout.
    layout: Layout,

    // Actuall data.
    payload: [u8],
}

struct Buffer(NonNull<BufferLayout>);

impl Buffer {
    const HEADER_BUFFER_FIELDS: &[Layout] =
        &[Layout::new::<Option<Buffer>>(), Layout::new::<Layout>(), Layout::new::<usize>()];

    pub fn alloc(size: usize) -> Buffer {
        // TODO(i.erin)
        let layout = Self::compute_layout(size).unwrap();

        // Safety: non zero layout size.
        assert!(layout.size() != 0);
        let ptr = unsafe { alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            alloc::handle_alloc_error(layout)
        }

        // Safety: non null, memory of size `size`, owning pointer
        // Discassed: https://users.rust-lang.org/t/can-i-create-a-reference-to-a-custom-dst-from-raw-parts-on-stable/63261
        assert!(!ptr.is_null());
        let ptr: *mut [()] = unsafe { std::slice::from_raw_parts_mut(ptr as *mut (), size) };

        // Rustonumicon: https://doc.rust-lang.org/stable/reference/expressions/operator-expr.html#pointer-to-pointer-cast
        let ptr = ptr as *mut BufferLayout;

        unsafe { (*ptr).next = None };
        unsafe { (*ptr).layout = layout };
        // payload already initialazed via alloca_zeroed

        // Safety: nullness checked right after allocation
        unsafe { Buffer(NonNull::new_unchecked(ptr)) }
    }

    // Safety: should be called only once for pointer created via [`Self::alloc`].
    pub unsafe fn dealloc(&mut self) {
        let ptr = self.0.as_ptr();

        let layout = unsafe { (*ptr).layout };
        unsafe { alloc::dealloc(ptr as *mut _, layout) };
    }

    pub fn as_mut(&mut self) -> &mut [u8] {
        self.as_mut_and_next().0
    }

    pub fn mut_next(&mut self) -> &mut Option<Buffer> {
        self.as_mut_and_next().1
    }

    pub fn as_mut_and_next(&mut self) -> (&mut [u8], &mut Option<Buffer>) {
        // Safety: TODO()
        let as_mut = unsafe { &mut (*self.0.as_ptr()).payload };
        let next = unsafe { &mut (*self.0.as_ptr()).next };

        (as_mut, next)
    }

    fn repr_c_layout(fields: &[Layout]) -> Result<Layout, LayoutError> {
        let mut layout = Layout::from_size_align(0, 1)?;
        for &field in fields {
            let (new_layout, _) = layout.extend(field)?;
            layout = new_layout;
        }
        Ok(layout.pad_to_align())
    }

    fn compute_layout(layout_size: usize) -> Result<Layout, LayoutError> {
        let header_layout = Self::repr_c_layout(Self::HEADER_BUFFER_FIELDS)?;
        let (layout, _) =
            header_layout.extend(Layout::for_value(vec![0u8; layout_size].as_slice()))?;

        Ok(layout.pad_to_align())
    }

    fn layout(&self) -> Layout {
        unsafe { (*self.0.as_ptr()).layout }
    }

    fn as_ptr(&self) -> *const BufferLayout {
        self.0.as_ptr()
    }
}

type Sender = mpsc::Sender<Buffer>;
type Reciever = mpsc::Receiver<Buffer>;

struct BufferChain {
    buffer: Option<Buffer>,
    sender: Sender,
}

impl BufferChain {
    pub fn new(sender: Sender) -> Self {
        Self { buffer: None, sender }
    }

    pub fn push(&mut self, mut buffer: Buffer) {
        assert!((*buffer.mut_next()).is_none());

        *buffer.mut_next() = self.buffer.take();
        self.buffer = Some(buffer)
    }

    // TODO(i.erin) implement Iterator for that
    pub fn to_vec(Self { buffer, .. }: &mut Self) -> Vec<&mut [u8]> {
        let mut result = Vec::new();

        let mut current = buffer;
        loop {
            match current {
                Some(buffer) => {
                    let (as_mut, next) = buffer.as_mut_and_next();
                    current = next;

                    result.push(as_mut)
                }
                None => return result,
            }
        }
    }

    pub fn blocking_dealloc(Self { buffer: mut next_buffer, sender }: Self) {
        while let Some(mut buffer) = next_buffer {
            next_buffer = buffer.mut_next().take();
            sender.blocking_send(buffer).expect("can't send buffer");
        }
    }

    pub async fn dealloc(Self { buffer: mut next_buffer, sender }: Self) {
        while let Some(mut buffer) = next_buffer {
            next_buffer = buffer.mut_next().take();
            sender.send(buffer).await.expect("can't send blocking");
        }
    }

    pub fn cursor(&mut self) -> Cursor<'_> {
        Cursor::new(self)
    }
}

/// Cursor for sequential read/write operations over a BufferChain.
pub struct Cursor<'a> {
    chain: &'a mut BufferChain,
    current_buffer: Option<&'a mut Buffer>,
    buffer_offset: usize,
    total_position: usize,
}

impl<'a> Cursor<'a> {
    fn new(chain: &'a mut BufferChain) -> Self {
        // Get reference to the first buffer
        let current_buffer = if let Some(ref mut buffer) = chain.buffer {
            // Safety: We're creating a mutable reference with proper lifetime
            // This is safe because we have exclusive access to the chain
            Some(unsafe { &mut *(buffer as *mut Buffer) })
        } else {
            None
        };

        Self { chain, current_buffer, buffer_offset: 0, total_position: 0 }
    }

    /// Returns the current position in the buffer chain.
    pub fn position(&self) -> usize {
        self.total_position
    }

    /// Returns the number of bytes remaining in the current buffer.
    pub fn remaining_in_buffer(&self) -> usize {
        self.current_buffer
            .as_ref()
            .map(|buf| {
                // Safety: We need to read the length without mutable access.
                // Making the reference explicit as required by the linter.
                //
                // TODO this lint is unfixable yet
                #[allow(clippy::needless_borrow)]
                let payload_len = unsafe { (&(*buf.0.as_ptr()).payload).len() };

                payload_len.saturating_sub(self.buffer_offset)
            })
            .unwrap_or(0)
    }

    /// Advances to the next buffer in the chain.
    fn advance_buffer(&mut self) -> bool {
        if let Some(current) = self.current_buffer.take() {
            let next_opt = current.mut_next();
            if let Some(next_buffer) = next_opt {
                // Safety: We're creating a mutable reference with proper lifetime
                // The chain owns the buffers, and we have exclusive access
                self.current_buffer = Some(unsafe { &mut *(next_buffer as *mut Buffer) });
                self.buffer_offset = 0;
                return true;
            }
        }
        false
    }

    /// Reads data into the provided buffer.
    /// Returns the number of bytes read.
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let mut total_read = 0;

        while total_read < buf.len() {
            if self.remaining_in_buffer() == 0 && !self.advance_buffer() {
                break;
            }

            if let Some(current) = &mut self.current_buffer {
                let buffer_data = current.as_mut();
                let available = buffer_data.len() - self.buffer_offset;
                let to_read = (buf.len() - total_read).min(available);

                buf[total_read..total_read + to_read].copy_from_slice(
                    &buffer_data[self.buffer_offset..self.buffer_offset + to_read],
                );

                self.buffer_offset += to_read;
                total_read += to_read;
                self.total_position += to_read;
            }
        }

        total_read
    }

    /// Writes data from the provided buffer.
    /// Returns the number of bytes written.
    pub fn write(&mut self, buf: &[u8]) -> usize {
        let mut total_written = 0;

        while total_written < buf.len() {
            if self.remaining_in_buffer() == 0 && !self.advance_buffer() {
                break;
            }

            if let Some(current) = &mut self.current_buffer {
                let buffer_data = current.as_mut();
                let available = buffer_data.len() - self.buffer_offset;
                let to_write = (buf.len() - total_written).min(available);

                buffer_data[self.buffer_offset..self.buffer_offset + to_write]
                    .copy_from_slice(&buf[total_written..total_written + to_write]);

                self.buffer_offset += to_write;
                total_written += to_write;
                self.total_position += to_write;
            }
        }

        total_written
    }

    /// Reads a single byte, returning None if at end of chain.
    pub fn read_byte(&mut self) -> Option<u8> {
        let mut byte = [0u8; 1];
        if self.read(&mut byte) == 1 {
            Some(byte[0])
        } else {
            None
        }
    }

    /// Writes a single byte, returning true if successful.
    pub fn write_byte(&mut self, byte: u8) -> bool {
        self.write(&[byte]) == 1
    }

    /// Reads an exact amount of bytes into the buffer.
    /// Returns true if successful, false if not enough data available.
    pub fn read_exact(&mut self, buf: &mut [u8]) -> bool {
        self.read(buf) == buf.len()
    }

    /// Writes all bytes from the buffer.
    /// Returns true if all bytes were written, false otherwise.
    pub fn write_all(&mut self, buf: &[u8]) -> bool {
        self.write(buf) == buf.len()
    }
}

struct Allocator {
    reciever: mpsc::Receiver<Buffer>,
    sender: mpsc::Sender<Buffer>,
    size: usize,
    count: usize,
}

impl Allocator {
    pub async fn new(size: usize, count: usize) -> Self {
        let (sender, reciever) = mpsc::channel::<Buffer>(count);

        for _ in 0..count {
            let buffer = Buffer::alloc(size);
            sender.send(buffer).await.expect("cannot init buffers");
        }

        Self { sender, reciever, size, count }
    }

    pub async fn alloc(&mut self, mut size: usize) -> BufferChain {
        assert!(size < self.size * self.count);

        let mut chain = BufferChain::new(self.sender.clone());
        while size != 0 {
            let buffer = self.reciever.recv().await.expect("channel not to be closed");

            chain.push(buffer);
            size = size.wrapping_sub(self.size);
        }

        chain
    }
}

// Checks that: we can use public [`Buffer`] api.
#[test]
fn payload_len() {
    const SIZE: usize = 12345;
    const VALUE: &[u8] = &[1, 2, 3, 4];

    let mut buffer = Buffer::alloc(SIZE);

    assert_eq!(buffer.as_mut().len(), SIZE);

    let mut_slice = buffer.as_mut();
    for (&value, buffer) in VALUE.iter().zip(mut_slice.iter_mut()) {
        *buffer = value;
    }
    assert_eq!(&mut_slice[0..VALUE.len()], VALUE);

    // Safety: same buffer, only called only once
    unsafe {
        buffer.dealloc();
    }
}

// Checks that: we can use public [`Buffer`] api.
#[test]
fn buffer_chain() {
    const BUFFER_COUNT: usize = 4;
    const SIZE: usize = 12345;
    const VALUE: &[u8] = &[1, 2, 3, 4];

    let (sender, _reciever) = mpsc::channel(BUFFER_COUNT);
    let mut buffer_chain = BufferChain::new(sender);

    buffer_chain.push(Buffer::alloc(SIZE));
    buffer_chain.push(Buffer::alloc(SIZE));

    let vec = BufferChain::to_vec(&mut buffer_chain);
    assert_eq!(vec.len(), 2);
}

#[test]
fn cursor_read_write() {
    const BUFFER_SIZE: usize = 100;
    const DATA1: &[u8] = b"Hello, World!";
    const DATA2: &[u8] = b"This is a test of the cursor implementation.";

    let (sender, _receiver) = mpsc::channel(4);
    let mut buffer_chain = BufferChain::new(sender);

    buffer_chain.push(Buffer::alloc(BUFFER_SIZE));
    buffer_chain.push(Buffer::alloc(BUFFER_SIZE));

    // Test writing
    let mut cursor = buffer_chain.cursor();
    assert_eq!(cursor.position(), 0);

    let written1 = cursor.write(DATA1);
    assert_eq!(written1, DATA1.len());
    assert_eq!(cursor.position(), DATA1.len());

    let written2 = cursor.write(DATA2);
    assert_eq!(written2, DATA2.len());
    assert_eq!(cursor.position(), DATA1.len() + DATA2.len());

    // Reset cursor for reading by creating a new one
    drop(cursor);
    let mut cursor = buffer_chain.cursor();

    // Test reading
    let mut read_buf1 = vec![0u8; DATA1.len()];
    assert!(cursor.read_exact(&mut read_buf1));
    assert_eq!(&read_buf1, DATA1);

    let mut read_buf2 = vec![0u8; DATA2.len()];
    assert!(cursor.read_exact(&mut read_buf2));
    assert_eq!(&read_buf2, DATA2);
}

#[test]
fn cursor_byte_operations() {
    const BUFFER_SIZE: usize = 50;

    let (sender, _receiver) = mpsc::channel(2);
    let mut buffer_chain = BufferChain::new(sender);

    buffer_chain.push(Buffer::alloc(BUFFER_SIZE));

    let mut cursor = buffer_chain.cursor();

    // Write individual bytes
    assert!(cursor.write_byte(b'A'));
    assert!(cursor.write_byte(b'B'));
    assert!(cursor.write_byte(b'C'));
    assert_eq!(cursor.position(), 3);

    // Reset and read
    drop(cursor);
    let mut cursor = buffer_chain.cursor();

    assert_eq!(cursor.read_byte(), Some(b'A'));
    assert_eq!(cursor.read_byte(), Some(b'B'));
    assert_eq!(cursor.read_byte(), Some(b'C'));
    assert_eq!(cursor.position(), 3);
}

#[test]
fn cursor_cross_buffer_boundary() {
    const BUFFER_SIZE: usize = 10;
    const LARGE_DATA: &[u8] = b"This data spans multiple buffers!";

    let (sender, _receiver) = mpsc::channel(4);
    let mut buffer_chain = BufferChain::new(sender);

    // Create multiple small buffers
    buffer_chain.push(Buffer::alloc(BUFFER_SIZE));
    buffer_chain.push(Buffer::alloc(BUFFER_SIZE));
    buffer_chain.push(Buffer::alloc(BUFFER_SIZE));
    buffer_chain.push(Buffer::alloc(BUFFER_SIZE));

    let mut cursor = buffer_chain.cursor();

    // Write data that crosses buffer boundaries
    let written = cursor.write(LARGE_DATA);
    assert_eq!(written, LARGE_DATA.len());

    // Reset and read back
    drop(cursor);
    let mut cursor = buffer_chain.cursor();

    let mut read_buf = vec![0u8; LARGE_DATA.len()];
    assert!(cursor.read_exact(&mut read_buf));
    assert_eq!(&read_buf, LARGE_DATA);
}
