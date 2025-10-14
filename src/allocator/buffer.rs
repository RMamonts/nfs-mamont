use std::alloc::{self, Layout, LayoutError};
use std::ptr::NonNull;

/// Instrusive linked list heap allocated node layout.
///
/// Node allocated with Global allocator.
#[repr(C)]
struct BufferLayout {
    // Pointer to the next entry of the intrusive linked list.
    next: Option<Buffer>,
    // Allocation layout.
    layout: Layout,
    // Actuall data.
    payload: [u8],
}

/// Pointer to the heap allocated intrusive linked list node.
pub struct Buffer(NonNull<BufferLayout>);

impl Buffer {
    /// Allocates intrusive linked list node ([`BufferLayout`]) and return pointer to it.
    pub fn alloc(size: usize) -> Self {
        let layout = Self::compute_layout(size).expect("to construct correct Layout");

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

    /// Deallocates buffer.
    ///
    /// # Safety: should be called only once for pointer created via [`Self::alloc`].
    pub unsafe fn dealloc(&mut self) {
        let ptr = self.0.as_ptr();

        let layout = unsafe { (*ptr).layout };
        unsafe { alloc::dealloc(ptr as *mut _, layout) };
    }

    /// Returns mutable slice to inner buffer.
    pub fn as_mut(&mut self) -> &mut [u8] {
        self.as_mut_and_next().0
    }

    /// Return shared slice to inner buffer.
    pub fn as_slice(&self) -> &[u8] {
        unsafe { &(*self.as_ptr()).payload }
    }

    /// Length of inner buffer.
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    /// Mutable reference to next linked list entry.
    pub fn mut_next(&mut self) -> &mut Option<Buffer> {
        self.as_mut_and_next().1
    }

    /// Mutable reference to inner buffer and next linked list entry.
    pub fn as_mut_and_next(&mut self) -> (&mut [u8], &mut Option<Buffer>) {
        // Safety:
        // We use signle linked list --- then user has now way to
        // obtain interleaved mutable references as `payload` never reachable
        // via `next` pointer.
        //
        // Then we return well align mutable references to non overlaping memory
        // regions with lifetime bounded to self.
        let as_mut = unsafe { &mut (*self.0.as_ptr()).payload };
        let next = unsafe { &mut (*self.0.as_ptr()).next };

        (as_mut, next)
    }

    /// Return raw pointer to heap allocated linked list node.
    fn as_ptr(&self) -> *const BufferLayout {
        self.0.as_ptr()
    }

    const HEADER_BUFFER_FIELDS: &[Layout] =
        &[Layout::new::<Option<Buffer>>(), Layout::new::<Layout>(), Layout::new::<usize>()];

    fn compute_layout(payload_size: usize) -> Result<Layout, LayoutError> {
        let mut layout = Layout::from_size_align(0, 1)?;

        for &field in Self::HEADER_BUFFER_FIELDS {
            let (new_layout, _) = layout.extend(field)?;
            layout = new_layout;
        }

        let payload_layout = Layout::array::<u8>(payload_size)?;
        let (full_layout, _) = layout.extend(payload_layout)?;

        Ok(full_layout.pad_to_align())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // Check that we can:
    // - allocate buffer
    // - write all of it
    // - read written values back
    // - deallocate buffer
    #[test]
    fn alloc_write_dealloc() {
        const SIZE: usize = 12345;

        let mut buffer = Buffer::alloc(SIZE);
        assert_eq!(buffer.as_mut().len(), SIZE);

        let value = (0..SIZE).map(|item| item as u8);
        for (value, buffer) in value.clone().zip(buffer.as_mut().iter_mut()) {
            *buffer = value;
        }
        assert!(buffer.as_mut().iter().enumerate().all(|(index, &value)| index as u8 == value));

        // Safety: same buffer, called only once.
        unsafe {
            buffer.dealloc();
        }
    }
}
