//! In-flight budget of one connection.
//!
//! The read task owns a [`Budget`] and takes a [`Token`] from it before it reads a
//! request. The token travels with the request through the global tasks and comes back
//! to the write task inside [`crate::task::InFlightReply`], which returns it with
//! [`Limiter::release`] once the reply bytes are handed to the socket. While all slots
//! are taken, [`Budget::acquire`] waits, so the socket stays unread and TCP flow control
//! holds the client back.
//!
//! The budget is counted rather than reference-counted to keep the hot path cheap: the
//! read task increments a local counter and compares it with the write task's progress,
//! which it re-reads only when it runs out of slots, and the write task does one atomic
//! increment per reply. The price is that a slot is freed only by returning its token:
//! a command dropped inside a closed global channel (server shutdown) keeps its slot,
//! and a read task parked on an exhausted budget then waits until the write task stops
//! ([`Limiter::close`]) or the runtime is dropped.

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::Notify;

/// Proof that one slot of a connection's budget is taken.
///
/// Returned with [`Limiter::release`] after the reply is written, or with
/// [`Budget::cancel`] when the record gets no reply. Dropping it keeps the slot taken.
#[must_use = "a dropped token keeps its in-flight slot taken"]
pub struct Token(());

/// Part of the budget shared with the write task.
pub struct Limiter {
    /// Tokens returned so far.
    released: AtomicU64,
    /// Set by the read task while it waits for a token to be returned.
    waiting: AtomicBool,
    /// Set once the write task is gone.
    closed: AtomicBool,
    /// Wakes the read task after a release or on close.
    notify: Notify,
}

impl Limiter {
    /// Frees the slot of `token`: its reply was handed to the socket.
    pub fn release(&self, token: Token) {
        let Token(()) = token;
        // `SeqCst` pairs with the loads in `Budget::acquire`: either the read task sees
        // this increment, or this load sees the read task waiting and wakes it.
        self.released.fetch_add(1, Ordering::SeqCst);
        if self.waiting.load(Ordering::SeqCst) {
            self.notify.notify_one();
        }
    }

    /// Tells the read task that the write task is gone, so that it stops instead of
    /// waiting for tokens that will never come back.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.notify.notify_one();
    }

    /// Returns the number of tokens returned so far.
    #[cfg(test)]
    pub fn released(&self) -> u64 {
        self.released.load(Ordering::SeqCst)
    }

    /// Returns whether [`Self::close`] was called.
    #[cfg(test)]
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
}

/// Read-task side of the budget: issues tokens and waits while all slots are taken.
pub struct Budget {
    limiter: Arc<Limiter>,
    /// Number of slots.
    limit: u64,
    /// Tokens issued so far.
    issued: u64,
    /// Last value of `Limiter::released` seen, refreshed only when the slots run out.
    released: u64,
}

impl Budget {
    /// Creates a budget of `limit` slots and the [`Limiter`] the write task returns
    /// tokens to.
    pub fn new(limit: NonZeroUsize) -> (Self, Arc<Limiter>) {
        let limiter = Arc::new(Limiter {
            released: AtomicU64::new(0),
            waiting: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            notify: Notify::new(),
        });
        let limit = u64::try_from(limit.get()).expect("usize fits into u64");
        let budget = Self { limiter: Arc::clone(&limiter), limit, issued: 0, released: 0 };

        (budget, limiter)
    }

    /// Takes a slot, waiting while all of them are taken.
    ///
    /// # Returns
    ///
    /// `None` once the write task is gone, see [`Limiter::close`].
    pub async fn acquire(&mut self) -> Option<Token> {
        loop {
            if self.has_slot() {
                break;
            }
            self.refresh();
            if self.has_slot() {
                break;
            }
            if self.limiter.closed.load(Ordering::SeqCst) {
                return None;
            }
            // Announce the wait, then look again: `Limiter::release` checks the flag
            // after its increment, so a release missed by this look finds the flag set
            // and wakes us. A notification stored before `notified()` is created still
            // completes it, and a stale one only costs another trip through the loop.
            self.limiter.waiting.store(true, Ordering::SeqCst);
            self.refresh();
            if !self.has_slot() && !self.limiter.closed.load(Ordering::SeqCst) {
                self.limiter.notify.notified().await;
            }
            self.limiter.waiting.store(false, Ordering::SeqCst);
        }
        self.issued += 1;

        Some(Token(()))
    }

    /// Gives `token` back for a record that gets no reply.
    pub fn cancel(&mut self, token: Token) {
        let Token(()) = token;
        self.issued -= 1;
    }

    fn has_slot(&self) -> bool {
        self.issued - self.released < self.limit
    }

    fn refresh(&mut self) {
        self.released = self.limiter.released.load(Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::time::Duration;

    use tokio::time::timeout;

    use super::{Budget, Token};

    const WAIT: Duration = Duration::from_millis(50);

    fn budget(limit: usize) -> (Budget, std::sync::Arc<super::Limiter>) {
        Budget::new(NonZeroUsize::new(limit).unwrap())
    }

    async fn take(budget: &mut Budget) -> Option<Token> {
        timeout(WAIT, budget.acquire()).await.expect("acquire should not wait")
    }

    #[tokio::test]
    async fn tokens_are_issued_up_to_the_limit() {
        let (mut budget, _limiter) = budget(2);

        let _first = take(&mut budget).await.unwrap();
        let _second = take(&mut budget).await.unwrap();
        assert!(timeout(WAIT, budget.acquire()).await.is_err(), "third token must wait");
    }

    #[tokio::test]
    async fn a_release_lets_the_next_acquire_through() {
        let (mut budget, limiter) = budget(1);
        let token = take(&mut budget).await.unwrap();

        let releaser = tokio::spawn(async move {
            tokio::time::sleep(WAIT).await;
            limiter.release(token);
        });
        let _token = timeout(WAIT * 10, budget.acquire()).await.expect("woken by the release");
        releaser.await.unwrap();
    }

    #[tokio::test]
    async fn a_cancelled_token_frees_its_slot() {
        let (mut budget, _limiter) = budget(1);
        let token = take(&mut budget).await.unwrap();

        budget.cancel(token);
        let _token = take(&mut budget).await.unwrap();
    }

    #[tokio::test]
    async fn close_stops_a_waiting_acquire() {
        let (mut budget, limiter) = budget(1);
        let _token = take(&mut budget).await.unwrap();

        let closer = tokio::spawn(async move {
            tokio::time::sleep(WAIT).await;
            limiter.close();
        });
        let next = timeout(WAIT * 10, budget.acquire()).await.expect("woken by the close");
        assert!(next.is_none());
        closer.await.unwrap();
    }

    #[tokio::test]
    async fn releases_are_seen_without_a_wait() {
        let (mut budget, limiter) = budget(1);
        let token = take(&mut budget).await.unwrap();

        limiter.release(token);
        let _token = take(&mut budget).await.unwrap();
        assert_eq!(limiter.released(), 1);
    }
}
