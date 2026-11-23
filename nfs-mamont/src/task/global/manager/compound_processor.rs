use std::future::Future;
use std::pin::Pin;

use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tokio::sync::mpsc::UnboundedReceiver;

pub type CompoundFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

pub struct CompoundProcessor {
    max_recv: usize,
    receiver: UnboundedReceiver<CompoundFuture>,
    futures: FuturesUnordered<CompoundFuture>,
}

impl CompoundProcessor {
    pub fn new(max_recv: usize, receiver: UnboundedReceiver<CompoundFuture>) -> Self {
        Self { receiver, futures: FuturesUnordered::new(), max_recv }
    }

    pub fn spawn(self) {
        tokio::task::spawn(async move { self.run().await });
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                Some(fut) = self.receiver.recv(), if self.futures.len() < self.max_recv  => {
                    self.futures.push(fut);
                },
                Some(_) = self.futures.next() => {}
            }
        }
    }
}
