//! Connections that have not sent a valid call yet, see
//! [`crate::ServerContext::with_max_unverified_connections`].
//!
//! A peer that connects and sends nothing, or nothing a call can be parsed from, holds a
//! socket, two tasks and their buffers for free. Only so many such connections are kept,
//! in their order of arrival: a new one pushes out the oldest, which is closed. A
//! connection leaves probation with its first valid call, so clients that talk are never
//! touched. The Linux NFS server limits such connections the same way.
//!
//! A connection that leaves probation stays in the queue: its place expires, and expired
//! places are skipped when the oldest connection is pushed out, and dropped once the queue
//! is full.

use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use tokio::sync::Notify;

/// Connections on probation, oldest first.
pub struct Probation {
    /// Eviction signal of every connection on probation, oldest first; a place whose
    /// [`Ticket`] is gone has expired.
    queue: Mutex<VecDeque<Weak<Notify>>>,
    max: usize,
}

impl Probation {
    /// Creates a probation of at most `max` connections.
    pub fn new(max: NonZeroUsize) -> Self {
        Self { queue: Mutex::new(VecDeque::new()), max: max.get() }
    }

    /// Puts a new connection on probation; when all places are taken, the oldest
    /// connection on probation is told to close.
    pub fn admit(&self) -> Ticket {
        let evicted = Arc::new(Notify::new());
        let mut queue = self.lock();
        if queue.len() == self.max {
            // Done only when the queue is full, so that it costs O(1) per connection.
            queue.retain(|place| place.strong_count() > 0);
            if queue.len() == self.max {
                evict_oldest(&mut queue);
            }
        }
        queue.push_back(Arc::downgrade(&evicted));
        drop(queue);

        Ticket { evicted }
    }

    /// Tells the oldest connection on probation to close, if there is one.
    pub fn evict_oldest(&self) {
        evict_oldest(&mut self.lock());
    }

    fn lock(&self) -> MutexGuard<'_, VecDeque<Weak<Notify>>> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Pops places up to the oldest one that has not expired and tells it to close.
fn evict_oldest(queue: &mut VecDeque<Weak<Notify>>) {
    while let Some(place) = queue.pop_front() {
        if let Some(evicted) = place.upgrade() {
            // Stores a permit if the connection is not waiting yet.
            evicted.notify_one();
            return;
        }
    }
}

/// Place of one connection on probation; dropping it takes the connection off.
pub struct Ticket {
    evicted: Arc<Notify>,
}

impl Ticket {
    /// Completes once newer connections have pushed this one out.
    pub async fn evicted(&self) {
        self.evicted.notified().await;
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::time::Duration;

    use tokio::time::timeout;

    use super::Probation;

    /// How long the tests wait to see that something does not happen.
    const WAIT: Duration = Duration::from_millis(50);

    #[tokio::test]
    async fn a_new_connection_pushes_out_the_oldest() {
        let probation = Probation::new(NonZeroUsize::new(2).unwrap());
        let (first, second) = (probation.admit(), probation.admit());
        let left = probation.admit();
        drop(left);
        let _third = probation.admit();

        timeout(WAIT, first.evicted()).await.expect("the oldest connection is pushed out");
        assert!(
            timeout(WAIT, second.evicted()).await.is_err(),
            "only one connection is pushed out"
        );
    }

    #[tokio::test]
    async fn connections_that_left_take_no_place() {
        let probation = Probation::new(NonZeroUsize::new(2).unwrap());
        let first = probation.admit();
        for _ in 0..10 {
            drop(probation.admit());
        }
        let _second = probation.admit();

        assert!(timeout(WAIT, first.evicted()).await.is_err(), "a place that expired was counted");
        assert!(probation.lock().len() <= 2, "expired places pile up");
    }

    #[tokio::test]
    async fn evicting_skips_connections_that_left() {
        let probation = Probation::new(NonZeroUsize::new(64).unwrap());
        drop(probation.admit());
        let second = probation.admit();

        probation.evict_oldest();
        timeout(WAIT, second.evicted()).await.expect("the oldest connection still on probation");
    }
}
