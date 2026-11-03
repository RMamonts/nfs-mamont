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
