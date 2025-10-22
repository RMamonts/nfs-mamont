use super::buffer::Buffer;
use super::Sender;

pub struct List {
    buffer: Option<Buffer>,
    sender: Sender,
}

struct IntoIter {
    next: Option<Buffer>,
}

impl Iterator for IntoIter {
    type Item = Buffer;

    fn next(&mut self) -> Option<Self::Item> {
        self.next.take().map(|mut buffer| {
            self.next = buffer.mut_next().take();
            buffer
        })
    }
}

pub struct IterMut<'a> {
    next: Option<&'a mut Buffer>,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut [u8];

    fn next(&mut self) -> Option<Self::Item> {
        self.next.take().map(|buffer| {
            let (payload, next) = buffer.as_mut_and_next();
            self.next = next.as_mut();

            payload
        })
    }
}

pub struct Iter<'a> {
    next: Option<&'a Buffer>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        self.next.take().map(|buffer| {
            let (payload, next) = buffer.as_slice_and_next();
            self.next = next.as_ref();

            payload
        })
    }
}

impl List {
    /// Crates new instrusive linked list of buffers.
    pub fn new(sender: Sender) -> Self {
        Self { buffer: None, sender }
    }

    /// Push new intrusive linked list node at head.
    pub fn push_head(&mut self, mut buffer: Buffer) {
        assert!((*buffer.mut_next()).is_none());

        *buffer.mut_next() = self.buffer.take();
        self.buffer = Some(buffer)
    }

    /// Deallocate in blocking manner.
    ///
    /// May deadlock in async context.
    pub fn blocking_dealloc(self) {
        let (buffers, sender) = self.into_iter();

        for mut buffer in buffers {
            assert!(buffer.mut_next().is_none());
            sender.blocking_send(buffer).expect("to send buffer")
        }
    }

    /// Deallocate in async manner.
    pub async fn dealloc(self) {
        let (buffers, sender) = self.into_iter();

        for mut buffer in buffers {
            assert!(buffer.mut_next().is_none());
            sender.send(buffer).await.expect("to send buffer")
        }
    }

    fn into_iter(self) -> (IntoIter, Sender) {
        (IntoIter { next: self.buffer }, self.sender)
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        IterMut { next: self.buffer.as_mut() }
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter { next: self.buffer.as_ref() }
    }
}
