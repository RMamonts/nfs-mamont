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

    fn compute_layout(payload_size: usize) -> Result<Layout, LayoutError> {
        let header_layout = Self::repr_c_layout(Self::HEADER_BUFFER_FIELDS)?;

        let payload_layout = Layout::array::<u8>(payload_size)?;
        let (full_layout, _) = header_layout.extend(payload_layout)?;

        Ok(full_layout.pad_to_align())
    }

    fn layout(&self) -> Layout {
        unsafe { (*self.0.as_ptr()).layout }
    }

    fn as_ptr(&self) -> *const BufferLayout {
        self.0.as_ptr()
    }
}

type Sender = mpsc::Sender<Buffer>;
type Receiver = mpsc::Receiver<Buffer>;

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
}

struct Allocator {
    Receiver,
    Sender,
    size: usize,
    count: usize,
}

impl Allocator {
    pub async fn new(size: usize, count: usize) -> Self {
        let (sender, receiver) = mpsc::channel::<Buffer>(count);

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
