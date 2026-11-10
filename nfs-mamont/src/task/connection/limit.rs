//! In-flight budget of one connection, see the [`super`] module docs.
//!
//! Slots are counted instead of taken from a semaphore, which costs about 4 % of GETATTR
//! throughput on four connections.

use std::io;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::Notify;

use crate::allocator::Buffer;
use crate::metric::{CountedSender, Timer};
use crate::rpc::Error;
use crate::task::ProcReply;

struct Limiter {
    /// Slots freed so far, wrapping.
    released: AtomicUsize,
    /// Wakes the read task after a slot is freed.
    notify: Notify,
}

/// One slot of a connection's budget, freed when dropped.
pub struct Token {
    limiter: Arc<Limiter>,
}

impl Drop for Token {
    fn drop(&mut self) {
        self.limiter.released.fetch_add(1, Ordering::Release);
        // Stores a permit if the read task is not parked yet, so the wake-up is not lost.
        self.limiter.notify.notify_one();
    }
}

/// A reply on its way to the write task, with the slot of its request.
pub struct InFlightReply<B: Buffer> {
    pub reply: ProcReply<B>,
    pub token: Token,
}

/// Answers one request: the channel to the write task of the connection and the slot
/// of the request.
pub struct ReplySender<B: Buffer> {
    tx: CountedSender<InFlightReply<B>>,
    token: Token,
}

impl<B: Buffer> ReplySender<B> {
    /// Queues `reply` for the write task.
    ///
    /// # Errors
    ///
    /// `BrokenPipe` when the write task is gone, i.e. the connection is closed.
    pub async fn send(self, reply: ProcReply<B>) -> io::Result<()> {
        self.tx
            .send(InFlightReply { reply, token: self.token })
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "write task closed"))
    }

    /// Answers `xid` with a `BrokenPipe` error because the global task that should have
    /// served it is gone (the server is shutting down); the connection is done after it.
    pub async fn broken_pipe(self, xid: u32) -> io::Result<()> {
        let error = io::Error::new(io::ErrorKind::BrokenPipe, "server task closed");
        self.send(ProcReply { xid, proc_result: Err(Error::IO(error)), timer: Timer::NONE }).await
    }
}

/// Read-task side of the limit: hands out one [`ReplySender`] per free slot.
pub struct Budget<B: Buffer> {
    limiter: Arc<Limiter>,
    limit: usize,
    /// Slots taken so far, wrapping.
    issued: usize,
    tx: CountedSender<InFlightReply<B>>,
}

impl<B: Buffer> Budget<B> {
    /// Creates a budget of `limit` slots for replies sent through `tx`.
    pub fn new(limit: NonZeroUsize, tx: CountedSender<InFlightReply<B>>) -> Self {
        let limiter = Arc::new(Limiter { released: AtomicUsize::new(0), notify: Notify::new() });

        Self { limiter, limit: limit.get(), issued: 0, tx }
    }

    /// Takes a slot, waiting while all of them are taken.
    pub async fn acquire(&mut self) -> ReplySender<B> {
        while self.issued.wrapping_sub(self.limiter.released.load(Ordering::Acquire)) == self.limit
        {
            self.limiter.notify.notified().await;
        }
        self.issued = self.issued.wrapping_add(1);

        ReplySender { tx: self.tx.clone(), token: Token { limiter: Arc::clone(&self.limiter) } }
    }
}
