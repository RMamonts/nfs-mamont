//! Defines list of [`Buffer`]'s.

use super::buffer::Buffer;

/// Represents list of [`Buffer`]'s.
pub struct List {
    buffers: Vec<Buffer>,
    sender: super::Sender<Buffer>,
}

impl List {
    /// Returns new list created of specified `buffers` and sent all of them to
    /// specified `sender` in order to reclaim.
    ///
    /// Assumes that all buffers have the same length.
    pub fn new(buffers: Vec<Buffer>, sender: super::Sender<Buffer>) -> Self {
        Self { buffers, sender }
    }

    /// Deallocates all buffers i.e. send them via specified in the [`Self::new`] `sender`.
    pub fn deallocate(&mut self) {
        for buffer in self.buffers.drain(..) {
            // Ignore allocator drop
            let _ = self.sender.send(buffer);
        }
    }

    /// Returns accumulated buffers length.
    pub fn len(&self) -> usize {
        // assume always one buffer for now
        self.buffers.len() * self.buffers[0].len()
    }
}

impl Drop for List {
    fn drop(&mut self) {
        self.deallocate();
    }
}

/// Unique iterator over [`List`].
///
/// Allows mutable access to each buffer in order of list.
pub struct IterMut<'a>(std::slice::IterMut<'a, Buffer>);

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut [u8];
    fn next(&mut self) -> Option<Self::Item> {
        use std::ops::DerefMut;

        self.0.next().map(DerefMut::deref_mut)
    }
}

impl<'a> IntoIterator for &'a mut List {
    type IntoIter = IterMut<'a>;
    type Item = &'a mut [u8];

    fn into_iter(self) -> Self::IntoIter {
        IterMut(self.buffers.iter_mut())
    }
}

/// Shared iterator over [`List`].
///
/// Allows read access to each buffer in order of list.
pub struct Iter<'a>(std::slice::Iter<'a, Buffer>);

impl<'a> Iterator for Iter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        use std::ops::Deref;

        self.0.next().map(Deref::deref)
    }
}

impl<'a> IntoIterator for &'a List {
    type IntoIter = Iter<'a>;
    type Item = &'a [u8];

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.buffers.iter())
    }
}
