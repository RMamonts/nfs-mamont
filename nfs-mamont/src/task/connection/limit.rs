//! In-flight budget of one connection, see the [`super`] module docs.
//!
//! Slots are counted instead of taken from a semaphore, which costs about 4 % of GETATTR
//! throughput on four connections.

use std::io;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_channel::Sender;
use tokio::sync::Notify;
use tokio::time::{Instant, Sleep};

use crate::allocator::Buffer;
use crate::rpc::Error;
use crate::task::ProcReply;

struct Limiter {
    /// Slots freed so far, wrapping.
    released: AtomicUsize,
    /// Bytes of READ and WRITE data freed so far, wrapping.
    bytes_released: AtomicUsize,
    /// Wakes the read task after a slot and its data are freed.
    notify: Notify,
}

/// One slot of a connection's budget and the data charged to it, freed when dropped.
pub struct Token {
    limiter: Arc<Limiter>,
    /// Bytes of data charged to the request, see [`Budget::charge`].
    bytes: usize,
}

impl Drop for Token {
    fn drop(&mut self) {
        if self.bytes > 0 {
            self.limiter.bytes_released.fetch_add(self.bytes, Ordering::Release);
        }
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
    tx: Sender<InFlightReply<B>>,
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
        self.send(ProcReply { xid, proc_result: Err(Error::IO(error)) }).await
    }
}

/// Read-task side of the limit: hands out one [`ReplySender`] per free slot.
pub struct Budget<B: Buffer> {
    limiter: Arc<Limiter>,
    limit: usize,
    /// Slots taken so far, wrapping.
    issued: usize,
    /// Limit on the data in flight, see [`Self::new`].
    data: Option<DataLimit>,
    /// Timer of [`Self::idle`], `None` without an idle timeout.
    idle: Option<IdleTimer>,
    tx: Sender<InFlightReply<B>>,
}

/// One timer per connection, re-armed at most once per period instead of on every
/// request.
struct IdleTimer {
    period: Duration,
    sleep: Pin<Box<Sleep>>,
    /// Slots issued and released when the timer was armed.
    armed_at: (usize, usize),
}

struct DataLimit {
    max: usize,
    /// Bytes charged so far, wrapping.
    issued: usize,
}

impl<B: Buffer> Budget<B> {
    /// Creates a budget of `limit` slots for replies sent through `tx`. With `max_data`
    /// it also stops reading while that many bytes of READ and WRITE data are in flight,
    /// see [`crate::ServerContext::with_max_data_per_connection`]; with `idle_timeout`,
    /// [`Self::idle`] tells when the connection has been idle that long.
    pub fn new(
        limit: NonZeroUsize,
        max_data: Option<NonZeroUsize>,
        idle_timeout: Option<Duration>,
        tx: Sender<InFlightReply<B>>,
    ) -> Self {
        let limiter = Arc::new(Limiter {
            released: AtomicUsize::new(0),
            bytes_released: AtomicUsize::new(0),
            notify: Notify::new(),
        });

        let data = max_data.map(|max| DataLimit { max: max.get(), issued: 0 });
        let idle = idle_timeout.map(|period| IdleTimer {
            period,
            sleep: Box::pin(tokio::time::sleep(period)),
            // `idle` is first polled with one slot issued, see its docs.
            armed_at: (1, 0),
        });

        Self { limiter, limit: limit.get(), issued: 0, data, idle, tx }
    }

    /// Takes a slot, waiting while all of them, or all the data, are taken.
    pub async fn acquire(&mut self) -> ReplySender<B> {
        while self.is_full() {
            self.limiter.notify.notified().await;
        }
        self.issued = self.issued.wrapping_add(1);

        let token = Token { limiter: Arc::clone(&self.limiter), bytes: 0 };
        ReplySender { tx: self.tx.clone(), token }
    }

    /// Charges `bytes` of data to the request `result_tx` answers; they are freed with
    /// its slot.
    pub fn charge(&mut self, result_tx: &mut ReplySender<B>, bytes: usize) {
        let Some(data) = &mut self.data else {
            return;
        };
        data.issued = data.issued.wrapping_add(bytes);
        result_tx.token.bytes += bytes;
    }

    /// Completes once the connection has had nothing in flight but the request being
    /// read, and no request read or answered, for one to two idle timeouts; never
    /// without an idle timeout. Called after [`Self::acquire`], while that request is
    /// read.
    pub async fn idle(&mut self) {
        let Some(timer) = &mut self.idle else {
            return std::future::pending().await;
        };
        loop {
            timer.sleep.as_mut().await;
            let now = (self.issued, self.limiter.released.load(Ordering::Acquire));
            if now == timer.armed_at && now.0.wrapping_sub(now.1) <= 1 {
                return;
            }
            // Something happened since the timer was armed, or a request is still in
            // flight: start over.
            timer.armed_at = now;
            timer.sleep.as_mut().reset(Instant::now() + timer.period);
        }
    }

    fn is_full(&self) -> bool {
        let in_flight = self.issued.wrapping_sub(self.limiter.released.load(Ordering::Acquire));
        let data_full = self.data.as_ref().is_some_and(|data| {
            let freed = self.limiter.bytes_released.load(Ordering::Acquire);
            data.issued.wrapping_sub(freed) >= data.max
        });

        (in_flight == self.limit) || data_full
    }
}
