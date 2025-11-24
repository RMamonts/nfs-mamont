use super::buffer::Buffer;

pub struct List {
    buffers: Vec<Buffer>,
    sender: super::Sender<Buffer>,
}

impl List {
    pub fn new(buffers: Vec<Buffer>, sender: super::Sender<Buffer>) -> Self {
        Self { buffers, sender }
    }

    pub fn deallocate(&mut self) {
        for buffer in self.buffers.drain(..) {
            // Ignore allocator drop
            let _ = self.sender.send(buffer);
        }
    }

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
