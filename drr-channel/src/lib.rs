//! Channel with one queue per sender, served by deficit round robin.
//!
//! [`channel`] returns a [`Sender`] and a [`Receiver`]. Every [`Sender::flow`] is a queue
//! of its own, a flow; receivers take items from the flows in turn, so that a flow gets
//! its share of the receivers however many items the others have queued. It is meant for
//! a pool of workers shared by many sources: connections, tenants, jobs.
//!
//! The flows that have items queued form a ring. A receiver takes the oldest item of the
//! flow at the head while the flow's deficit covers its [`Cost`]; otherwise the flow is
//! credited one more quantum and moves to the tail. A flow that runs empty leaves the
//! ring and loses its deficit, so an idle flow banks no credit. A flow alone gets every
//! receiver, backlogged flows get equal shares of the cost, and a light flow waits for
//! at most one quantum of each heavy one.
//!
//! At most `capacity` items may be queued or still held by receivers at once, see
//! [`Slot`]; senders wait for a slot in the order they came.
//!
//! # Choosing the quantum
//!
//! The quantum is how many cost units a flow may spend per turn. A light flow waits for
//! one quantum of every backlogged one, so a small quantum isolates flows better; but the
//! items of one flow then no longer follow each other, which costs whatever the receivers
//! gain from serving a flow in a row. An item costing more than the quantum takes several
//! turns of the ring.
//!
//! # Example
//!
//! ```
//! use std::num::{NonZeroU32, NonZeroUsize};
//!
//! use drr_channel::{channel, Cost};
//!
//! #[derive(Debug)]
//! struct Job(u32);
//!
//! impl Cost for Job {
//!     fn cost(&self) -> u32 {
//!         1
//!     }
//! }
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() {
//! let capacity = NonZeroUsize::new(64).unwrap();
//! let quantum = NonZeroU32::new(2).unwrap();
//! let (sender, receiver) = channel(capacity, quantum);
//!
//! let (busy, quiet) = (sender.flow(), sender.flow());
//! for id in 0..4 {
//!     busy.send(Job(id)).await.unwrap();
//! }
//! quiet.send(Job(100)).await.unwrap();
//!
//! // The busy flow spends its quantum, then the quiet one gets its turn.
//! let mut order = Vec::new();
//! for _ in 0..5 {
//!     let (job, _slot) = receiver.recv().await.unwrap();
//!     order.push(job.0);
//! }
//! assert_eq!(order, [0, 1, 100, 2, 3]);
//! # }
//! ```

#[cfg(test)]
mod tests;

use std::collections::VecDeque;
use std::num::{NonZeroU32, NonZeroUsize};
use std::pin::pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

/// How much of the receivers' work an item takes, in units of the cheapest item.
pub trait Cost {
    /// Returns the cost of the item, at least 1.
    fn cost(&self) -> u32;
}

/// Creates a channel holding at most `capacity` items, whose flows may spend `quantum`
/// cost units per turn.
pub fn channel<T: Cost>(capacity: NonZeroUsize, quantum: NonZeroU32) -> (Sender<T>, Receiver<T>) {
    // `Semaphore::new` panics above `MAX_PERMITS`.
    let slots = Arc::new(Semaphore::new(capacity.get().min(Semaphore::MAX_PERMITS)));
    let inner = Inner {
        flows: Vec::new(),
        free: Vec::new(),
        ring: VecDeque::new(),
        quantum: quantum.get(),
        closed: false,
    };
    let shared = Arc::new(Shared { inner: Mutex::new(inner), wake: Notify::new(), slots });

    (Sender { shared: Arc::clone(&shared) }, Receiver { shared })
}

/// Opens flows; dropping it closes the channel.
pub struct Sender<T> {
    shared: Arc<Shared<T>>,
}

impl<T> Sender<T> {
    /// Opens a new flow.
    ///
    /// Every flow gets a share of its own, so one source of items should hold one flow;
    /// that is why [`FlowSender`] is not `Clone`.
    pub fn flow(&self) -> FlowSender<T> {
        let mut inner = self.shared.lock();
        let flow = Flow { queue: VecDeque::new(), deficit: 0, dropped: false };
        let id = match inner.free.pop() {
            Some(id) => {
                inner.flows[id] = Some(flow);
                id
            }
            None => {
                inner.flows.push(Some(flow));
                inner.flows.len() - 1
            }
        };
        drop(inner);

        FlowSender { shared: Arc::clone(&self.shared), id }
    }
}

impl<T> Drop for Sender<T> {
    /// Closes the channel: the items already queued are still handed out, then
    /// [`Receiver::recv`] returns `None`, and [`FlowSender::send`] fails.
    fn drop(&mut self) {
        self.shared.lock().closed = true;
        self.shared.wake.notify_waiters();
    }
}

/// Sends the items of one flow; dropping it closes the flow once its items are taken.
pub struct FlowSender<T> {
    shared: Arc<Shared<T>>,
    id: usize,
}

impl<T> FlowSender<T> {
    /// Queues `item` on this flow, first waiting for a slot of the capacity.
    ///
    /// The slots are handed out in the order senders asked for them, so when every flow
    /// has one sender waiting at a time, they go round the waiting flows in turn.
    ///
    /// # Errors
    ///
    /// Gives the item back when the channel is closed.
    pub async fn send(&self, item: T) -> Result<(), T> {
        // The semaphore is never closed, so the error only stands for a closed channel.
        let Ok(slot) = Arc::clone(&self.shared.slots).acquire_owned().await else {
            return Err(item);
        };
        {
            let mut guard = self.shared.lock();
            let inner = &mut *guard;
            if inner.closed {
                return Err(item);
            }
            let flow = inner.flows[self.id].as_mut().expect("a sender keeps its flow open");
            if flow.queue.is_empty() {
                // A flow joining the ring starts a fresh turn: what it left unspent
                // before is lost, so an idle flow banks no credit.
                flow.deficit = inner.quantum;
                inner.ring.push_back(self.id);
            }
            flow.queue.push_back((item, Slot { _permit: slot }));
        }
        self.shared.wake.notify_one();

        Ok(())
    }
}

impl<T> Drop for FlowSender<T> {
    fn drop(&mut self) {
        let mut inner = self.shared.lock();
        let flow = inner.flows[self.id].as_mut().expect("a sender keeps its flow open");
        if flow.queue.is_empty() {
            inner.release(self.id);
        } else {
            flow.dropped = true;
        }
    }
}

/// Takes items from all flows in deficit round robin order.
pub struct Receiver<T> {
    shared: Arc<Shared<T>>,
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        Self { shared: Arc::clone(&self.shared) }
    }
}

impl<T: Cost> Receiver<T> {
    /// Takes the next item, waiting while none is queued. The item counts against the
    /// capacity for as long as its [`Slot`] is held.
    ///
    /// # Returns
    ///
    /// `None` once the channel is closed and drained.
    pub async fn recv(&self) -> Option<(T, Slot)> {
        loop {
            // Registered before looking, so that an item queued in between wakes us.
            let mut notified = pin!(self.shared.wake.notified());
            notified.as_mut().enable();
            {
                let mut inner = self.shared.lock();
                if let Some(picked) = inner.pick() {
                    return Some(picked);
                }
                if inner.closed {
                    return None;
                }
            }
            notified.await;
        }
    }
}

/// Counters of the channel, for metrics and tests. Each of them takes the lock of the
/// channel and looks at every flow.
impl<T> Receiver<T> {
    /// Returns the number of items queued over all flows.
    pub fn queued(&self) -> usize {
        self.shared.lock().flows.iter().flatten().map(|flow| flow.queue.len()).sum()
    }

    /// Returns the number of open flows, including dropped ones not yet drained.
    pub fn flows(&self) -> usize {
        self.shared.lock().flows.iter().flatten().count()
    }

    /// Returns the number of flows whose sender is still there.
    pub fn senders(&self) -> usize {
        self.shared.lock().flows.iter().flatten().filter(|flow| !flow.dropped).count()
    }
}

/// Slot of one item in the capacity of the channel, freed when dropped.
pub struct Slot {
    _permit: OwnedSemaphorePermit,
}

struct Shared<T> {
    inner: Mutex<Inner<T>>,
    /// Wakes a receiver when an item is queued or the channel closes.
    wake: Notify,
    /// Capacity of the channel.
    slots: Arc<Semaphore>,
}

impl<T> Shared<T> {
    fn lock(&self) -> MutexGuard<'_, Inner<T>> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Queue of one flow; it is in the ring exactly while it holds items.
struct Flow<T> {
    queue: VecDeque<(T, Slot)>,
    /// Cost units the flow may still spend in its turn.
    deficit: u32,
    /// Whether its sender is gone; the flow is freed once its queue is drained.
    dropped: bool,
}

struct Inner<T> {
    /// Flows by id; `None` marks an id that is free for the next flow.
    flows: Vec<Option<Flow<T>>>,
    free: Vec<usize>,
    /// Flows with queued items; the head is served next.
    ring: VecDeque<usize>,
    /// Cost units a flow may spend per turn.
    quantum: u32,
    closed: bool,
}

impl<T> Inner<T> {
    fn release(&mut self, id: usize) {
        self.flows[id] = None;
        self.free.push(id);
    }
}

impl<T: Cost> Inner<T> {
    /// Takes the next item in deficit round robin order.
    fn pick(&mut self) -> Option<(T, Slot)> {
        loop {
            let id = *self.ring.front()?;
            let flow = self.flows[id].as_mut().expect("flows in the ring are open");
            let (oldest, _) = flow.queue.front().expect("flows in the ring have queued items");
            let cost = oldest.cost();
            if flow.deficit < cost {
                // Too little left for the oldest item: one more quantum, the others first.
                flow.deficit += self.quantum;
                self.ring.pop_front();
                self.ring.push_back(id);
                continue;
            }
            flow.deficit -= cost;
            let picked = flow.queue.pop_front();
            if flow.queue.is_empty() {
                // Served out: leave the ring.
                let dropped = flow.dropped;
                self.ring.pop_front();
                if dropped {
                    self.release(id);
                }
            }
            return picked;
        }
    }
}
