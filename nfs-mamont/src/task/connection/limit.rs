//! In-flight budget of one connection.
//!
//! The read task owns a [`Budget`] and takes a [`Token`] from it before it reads a
//! request. The token travels with the request through the global tasks and comes back
//! to the write task inside [`crate::task::InFlightReply`], which returns it with
//! [`Limiter::release`] once the reply bytes are handed to the socket. While all slots
//! are taken, [`Budget::acquire`] waits, so the socket stays unread and TCP flow control
//! holds the client back.
//!
//! Slots are counted instead of taken from a semaphore to keep the hot path cheap: the
//! read task increments a local counter and compares it with the number of freed slots,
//! which it re-reads only when it runs out of slots, and every freed slot costs one
//! atomic increment. A token holds the [`Limiter`] and frees its slot when it is
//! dropped, so a request dropped anywhere on its way (a panicking backend, a closed
//! global channel) cannot leak a slot and leave the read task waiting forever.

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::Notify;

/// Proof that one slot of a connection's budget is taken.
///
/// The slot is freed when the token is dropped: by the write task once the reply is
/// written ([`Limiter::release`]), by the read task for a record that gets no reply
/// ([`Budget::cancel`]), or wherever the request is dropped on its way.
#[must_use = "dropping a token frees its in-flight slot right away"]
pub struct Token {
    limiter: Arc<Limiter>,
}

impl Drop for Token {
    fn drop(&mut self) {
        self.limiter.free_slot();
    }
}

/// Part of the budget shared with the write task and the issued tokens.
pub struct Limiter {
    /// Slots freed so far.
    released: AtomicU64,
    /// Set by the read task while it waits for a slot to be freed.
    waiting: AtomicBool,
    /// Set once the write task is gone.
    closed: AtomicBool,
    /// Wakes the read task after a slot is freed or on close.
    notify: Notify,
}

impl Limiter {
    /// Frees the slot of `token`: its reply was handed to the socket.
    ///
    /// Same as dropping the token, spelled out where the reply is written.
    pub fn release(&self, token: Token) {
        debug_assert!(
            std::ptr::eq(self, Arc::as_ptr(&token.limiter)),
            "Limiter::release: the token belongs to another connection"
        );
        drop(token);
    }

    /// Tells the read task that the write task is gone, so that it stops instead of
    /// waiting for slots that will never be freed.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.notify.notify_one();
    }

    fn free_slot(&self) {
        // `SeqCst` pairs with the loads in `Budget::acquire`: either the read task sees
        // this increment, or this load sees the read task waiting and wakes it.
        self.released.fetch_add(1, Ordering::SeqCst);
        if self.waiting.load(Ordering::SeqCst) {
            self.notify.notify_one();
        }
    }

    /// Returns the number of slots freed so far.
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
            // Announce the wait, then look again: a freed slot checks the flag after
            // its increment, so a slot missed by this look finds the flag set and wakes
            // us. A notification stored before `notified()` is created still completes
            // it, and a stale one only costs another trip through the loop.
            self.limiter.waiting.store(true, Ordering::SeqCst);
            self.refresh();
            if !self.has_slot() && !self.limiter.closed.load(Ordering::SeqCst) {
                self.limiter.notify.notified().await;
            }
            self.limiter.waiting.store(false, Ordering::SeqCst);
        }
        self.issued += 1;

        Some(Token { limiter: Arc::clone(&self.limiter) })
    }

    /// Gives `token` back for a record that gets no reply.
    pub fn cancel(&mut self, token: Token) {
        drop(token);
    }

    fn has_slot(&self) -> bool {
        self.issued - self.released < self.limit
    }

    fn refresh(&mut self) {
        self.released = self.limiter.released.load(Ordering::SeqCst);
    }
}
