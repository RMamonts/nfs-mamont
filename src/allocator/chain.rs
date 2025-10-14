use super::buffer::Buffer;
use super::Sender;

pub struct Chain {
    buffer: Option<Buffer>,
    sender: Sender,
}

impl Chain {
    /// Crates new instrusive linked chain of buffers.
    pub fn new(sender: Sender) -> Self {
        Self { buffer: None, sender }
    }

    /// Push new intrusive linked list node at head.
    pub fn push_head(&mut self, mut buffer: Buffer) {
        assert!((*buffer.mut_next()).is_none());

        *buffer.mut_next() = self.buffer.take();
        self.buffer = Some(buffer)
    }

    /// Represent linked list chain as vector of mutable slices.
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

    /// Deallocate in blocking manner.
    ///
    /// May deadlock in async context.
    pub fn blocking_dealloc(Self { buffer: mut next_buffer, sender }: Self) {
        while let Some(mut buffer) = next_buffer {
            next_buffer = buffer.mut_next().take();
            sender.blocking_send(buffer).expect("can't send blocking");
        }
    }

    /// Deallocate in async manner.
    pub async fn dealloc(Self { buffer: mut next_buffer, sender }: Self) {
        while let Some(mut buffer) = next_buffer {
            next_buffer = buffer.mut_next().take();
            sender.send(buffer).await.expect("can't send async");
        }
    }
}
