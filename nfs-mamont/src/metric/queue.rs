//! Channels whose depth is tracked as a [`Queue`](super::Queue) gauge.
//!
//! With the `metrics` feature disabled, these are plain `async_channel` types.

pub use counted::*;

#[cfg(feature = "metrics")]
mod counted {
    use std::sync::Arc;

    use async_channel::{Receiver, RecvError, SendError, Sender};

    use crate::metric::{NfsMetrics, Queue};

    /// Creates an unbounded channel whose depth is tracked as `queue`
    pub fn unbounded<T>(
        metrics: Arc<NfsMetrics>,
        queue: Queue,
    ) -> (CountedSender<T>, CountedReceiver<T>) {
        let (tx, rx) = async_channel::unbounded();
        let receiver =
            CountedReceiver(Arc::new(Inner { rx, metrics: Arc::clone(&metrics), queue }));
        (CountedSender { tx, metrics, queue }, receiver)
    }

    /// Channel sender that counts every message in the `queue` depth gauge
    pub struct CountedSender<T> {
        tx: Sender<T>,
        metrics: Arc<NfsMetrics>,
        queue: Queue,
    }

    impl<T> CountedSender<T> {
        pub async fn send(&self, msg: T) -> Result<(), SendError<T>> {
            // todo: for bounded, should decrement if sender gets cancelled while waiting for capacity
            self.metrics.enqueued(self.queue);
            self.tx.send(msg).await.map_err(|err| {
                self.metrics.dequeued(self.queue, 1);
                err
            })
        }

        pub fn close(&self) -> bool {
            self.tx.close()
        }
    }

    impl<T> Clone for CountedSender<T> {
        fn clone(&self) -> Self {
            Self { tx: self.tx.clone(), metrics: Arc::clone(&self.metrics), queue: self.queue }
        }
    }

    /// Channel receiver that uncounts received messages, and messages left in the channel once the last clone is dropped
    pub struct CountedReceiver<T>(Arc<Inner<T>>);

    struct Inner<T> {
        rx: Receiver<T>,
        metrics: Arc<NfsMetrics>,
        queue: Queue,
    }

    impl<T> CountedReceiver<T> {
        pub async fn recv(&self) -> Result<T, RecvError> {
            let msg = self.0.rx.recv().await?;
            self.0.metrics.dequeued(self.0.queue, 1);
            Ok(msg)
        }
    }

    impl<T> Clone for CountedReceiver<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }

    impl<T> Drop for Inner<T> {
        fn drop(&mut self) {
            // make sure noone pushes after reading len
            self.rx.close();
            self.metrics.dequeued(self.queue, self.rx.len());
        }
    }
}

#[cfg(not(feature = "metrics"))]
mod counted {
    use std::sync::Arc;

    use crate::metric::{NfsMetrics, Queue};

    pub type CountedSender<T> = async_channel::Sender<T>;
    pub type CountedReceiver<T> = async_channel::Receiver<T>;

    pub fn unbounded<T>(
        _metrics: Arc<NfsMetrics>,
        _queue: Queue,
    ) -> (CountedSender<T>, CountedReceiver<T>) {
        async_channel::unbounded()
    }
}
