//! Connections that have not sent a valid call yet, see
//! [`crate::ServerContext::with_max_unverified_connections`].
//!
//! A peer that connects and sends nothing, or nothing a call can be parsed from, holds a
//! socket, two tasks and their buffers for free. Only so many such connections are kept,
//! in their order of arrival: a new one pushes out the oldest, which is closed. A
//! connection leaves probation with its first valid call, so clients that talk are never
//! touched. The Linux NFS server limits such connections the same way.

use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::Notify;

/// Connections on probation, oldest first.
pub struct Probation {
    /// Eviction signal of every connection on probation, oldest first.
    queue: Mutex<VecDeque<Arc<Notify>>>,
    max: usize,
}

impl Probation {
    /// Creates a probation of at most `max` connections.
    pub fn new(max: NonZeroUsize) -> Arc<Self> {
        Arc::new(Self { queue: Mutex::new(VecDeque::new()), max: max.get() })
    }

    /// Puts a new connection on probation; when all places are taken, the oldest
    /// connection on probation is told to close.
    pub fn admit(self: &Arc<Self>) -> Ticket {
        let evicted = Arc::new(Notify::new());
        let mut queue = self.lock();
        if queue.len() == self.max {
            evict_oldest(&mut queue);
        }
        queue.push_back(Arc::clone(&evicted));
        drop(queue);

        Ticket { probation: Arc::clone(self), evicted }
    }

    /// Tells the oldest connection on probation to close, if there is one.
    pub fn evict_oldest(&self) {
        evict_oldest(&mut self.lock());
    }

    fn lock(&self) -> MutexGuard<'_, VecDeque<Arc<Notify>>> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn evict_oldest(queue: &mut VecDeque<Arc<Notify>>) {
    if let Some(oldest) = queue.pop_front() {
        // Stores a permit if the connection is not waiting yet.
        oldest.notify_one();
    }
}

/// Place of one connection on probation; dropping it takes the connection off.
pub struct Ticket {
    probation: Arc<Probation>,
    evicted: Arc<Notify>,
}

impl Ticket {
    /// Completes once newer connections have pushed this one out.
    pub async fn evicted(&self) {
        self.evicted.notified().await;
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let mut queue = self.probation.lock();
        if let Some(position) = queue.iter().position(|place| Arc::ptr_eq(place, &self.evicted)) {
            queue.remove(position);
        }
    }
}
