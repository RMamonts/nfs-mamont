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
            let _ = sender.send(buffer).await;
        }
    }
}

    #[cfg(test)]
    mod tests {
        use super::*;
        use tokio::sync::mpsc;

        #[test]
        fn push_head_and_to_vec_preserve_lifo_order() {
        let (sender, mut receiver) = mpsc::channel(2);
        let mut chain = Chain::new(sender);

            let mut first = Buffer::alloc(4);
            first.as_mut().copy_from_slice(&[1, 2, 3, 4]);

            let mut second = Buffer::alloc(4);
            second.as_mut().copy_from_slice(&[10, 11, 12, 13]);

            chain.push_head(first);
            chain.push_head(second);

            {
                let slices = Chain::to_vec(&mut chain);
                assert_eq!(slices.len(), 2);
                assert_eq!(slices[0].to_vec(), vec![10, 11, 12, 13]);
                assert_eq!(slices[1].to_vec(), vec![1, 2, 3, 4]);
            }

            Chain::blocking_dealloc(chain);

            let mut recovered = Vec::new();
            while let Ok(mut buffer) = receiver.try_recv() {
                assert!(buffer.mut_next().is_none());
                recovered.push(buffer);
            }
            assert_eq!(recovered.len(), 2);

            for mut buffer in recovered {
                unsafe { buffer.dealloc() };
            }
        }

        #[test]
        #[should_panic]
        fn push_head_panics_when_buffer_already_linked() {
            let (sender, _receiver) = mpsc::channel(1);
            let mut chain = Chain::new(sender);

            let mut buffer = Buffer::alloc(8);
            let tail = Buffer::alloc(8);
            *buffer.mut_next() = Some(tail);

            chain.push_head(buffer);
        }

        #[tokio::test(flavor = "current_thread")]
        async fn async_dealloc_returns_all_buffers() {
        let (sender, mut receiver) = mpsc::channel(3);
        let mut chain = Chain::new(sender);

            chain.push_head(Buffer::alloc(5));
            chain.push_head(Buffer::alloc(7));
            chain.push_head(Buffer::alloc(9));

            Chain::dealloc(chain).await;

            let mut recovered = Vec::new();
            while let Some(buffer) = receiver.recv().await {
                recovered.push(buffer);
                if recovered.len() == 3 {
                    break;
                }
            }
            assert_eq!(recovered.len(), 3);

            for mut buffer in recovered {
                unsafe { buffer.dealloc() };
            }
        }
    }
