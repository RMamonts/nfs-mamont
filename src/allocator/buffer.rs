use std::alloc::{self, Layout, LayoutError};
use std::num::NonZeroUsize;

/// Instrusive linked list heap allocated node layout.
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
pub struct Buffer(Box<BufferLayout>);

impl Buffer {
    /// Allocates intrusive linked list node ([`BufferLayout`]) and return pointer to it.
    pub fn alloc(size: NonZeroUsize) -> Self {
        let layout = Self::compute_layout(size.get()).expect("to construct correct Layout");

        // Safety: non zero layout size.
        assert!(layout.size() != 0);
        let ptr = unsafe { alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            alloc::handle_alloc_error(layout)
        }

        // Safety: non null, memory of size `size`, owning pointer
        // Discassed: https://users.rust-lang.org/t/can-i-create-a-reference-to-a-custom-dst-from-raw-parts-on-stable/63261
        assert!(!ptr.is_null());
        let ptr: *mut [()] = unsafe { std::slice::from_raw_parts_mut(ptr as *mut (), size.get()) };

        // Rustonumicon: https://doc.rust-lang.org/stable/reference/expressions/operator-expr.html#pointer-to-pointer-cast
        let ptr = ptr as *mut BufferLayout;

        unsafe { (*ptr).next = None };
        unsafe { (*ptr).layout = layout };
        // payload already initialazed via alloca_zeroed

        // Safety: we allocate by Global allocator with
        // right layout, memory initialized.
        unsafe { Buffer(Box::from_raw(ptr)) }
    }

    /// Returns mutable slice to inner buffer.
    pub fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0.payload
    }

    /// Return shared slice to inner buffer.
    pub fn as_slice(&self) -> &[u8] {
        &self.0.payload
    }

    /// Length of inner buffer.
    pub fn len(&self) -> usize {
        self.0.payload.len()
    }

    /// Mutable reference to next linked list entry.
    pub fn mut_next(&mut self) -> &mut Option<Buffer> {
        &mut self.0.next
    }

    pub fn as_slice_and_next(&self) -> (&[u8], &Option<Buffer>) {
        (&self.0.payload, &self.0.next)
    }

    pub fn as_mut_and_next(&mut self) -> (&mut [u8], &mut Option<Buffer>) {
        (&mut self.0.payload, &mut self.0.next)
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

impl Drop for Buffer {
    fn drop(&mut self) {
        assert!(self.mut_next().is_none())
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

        let mut buffer = Buffer::alloc(NonZeroUsize::new(SIZE).unwrap());
        assert!(buffer.mut_next().is_none());
        assert_eq!(buffer.as_mut().len(), SIZE);

        let value = (0..SIZE).map(|item| item as u8);
        for (value, buffer) in value.clone().zip(buffer.as_mut().iter_mut()) {
            *buffer = value;
        }
        assert!(buffer.as_mut().iter().enumerate().all(|(index, &value)| index as u8 == value));

        drop(buffer)
    }
}
