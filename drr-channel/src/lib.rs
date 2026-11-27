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
//! [`Slot`]; senders wait for a slot in the order they came, at most one per flow, so the
//! waiting flows take the slots in turn.
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
//! let (mut busy, mut quiet) = (sender.flow(), sender.flow());
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
    /// Returns the cost of the item; 0 counts as 1.
    ///
    /// Called once, when the item is sent.
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
        quantum: u64::from(quantum.get()),
        queued: 0,
        senders: 0,
        receivers: 1,
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
        inner.senders += 1;
        drop(inner);

        FlowSender { shared: Arc::clone(&self.shared), id }
    }
}

impl<T> Drop for Sender<T> {
    /// Closes the channel: the items already queued are still handed out, then
    /// [`Receiver::recv`] returns `None`, and [`FlowSender::send`] fails, also while it
    /// waits for a slot.
    fn drop(&mut self) {
        self.shared.close();
    }
}

/// Sends the items of one flow; dropping it closes the flow once its items are taken.
pub struct FlowSender<T> {
    shared: Arc<Shared<T>>,
    id: usize,
}

impl<T: Cost> FlowSender<T> {
    /// Queues `item` on this flow, first waiting for a slot of the capacity.
    ///
    /// The slots are handed out in the order senders asked for them. Taking `&mut self`
    /// keeps a flow to one waiting sender, so the slots go round the waiting flows in
    /// turn, however many items each of them has to send.
    ///
    /// # Errors
    ///
    /// Gives the item back when the channel is closed: the [`Sender`] or every
    /// [`Receiver`] is gone.
    pub async fn send(&mut self, item: T) -> Result<(), T> {
        // Outside the lock, and once: the receivers go by this value.
        let cost = item.cost().max(1);
        // The semaphore is closed with the channel.
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

            flow.queue.push_back(Queued { item, cost, slot: Slot { _permit: slot } });
            inner.queued += 1;
        }

        self.shared.wake.notify_one();

        Ok(())
    }
}

impl<T> Drop for FlowSender<T> {
    fn drop(&mut self) {
        let mut inner = self.shared.lock();
        inner.senders -= 1;
        let flow = inner.flows[self.id].as_mut().expect("a sender keeps its flow open");
        if flow.queue.is_empty() {
            inner.release(self.id);
        } else {
            flow.dropped = true;
        }
    }
}

/// Takes items from all flows in deficit round robin order; dropping the last one closes
/// the channel and drops the queued items.
pub struct Receiver<T> {
    shared: Arc<Shared<T>>,
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        self.shared.lock().receivers += 1;
        Self { shared: Arc::clone(&self.shared) }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let queued = {
            let mut inner = self.shared.lock();
            inner.receivers -= 1;

            if inner.receivers > 0 {
                return;
            }

            // Under the same lock as the drain, so that no sender queues after it.
            inner.closed = true;
            inner.drain()
        };

        self.shared.close();
        // Dropped outside the lock: an item may do anything when dropped.
        drop(queued);
    }
}

impl<T> Receiver<T> {
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
/// channel for a moment.
impl<T> Receiver<T> {
    /// Returns the number of items queued over all flows.
    pub fn queued(&self) -> usize {
        self.shared.lock().queued
    }

    /// Returns the number of open flows, including dropped ones not yet drained.
    pub fn flows(&self) -> usize {
        self.shared.lock().open()
    }

    /// Returns the number of flows whose sender is still there.
    pub fn senders(&self) -> usize {
        self.shared.lock().senders
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

    /// Closes the channel: senders fail, also those waiting for a slot, and receivers
    /// return `None` once the queues are drained.
    fn close(&self) {
        self.lock().closed = true;
        self.slots.close();
        self.wake.notify_waiters();
    }
}

/// Item waiting in the queue of its flow.
struct Queued<T> {
    item: T,
    /// [`Cost::cost`] of the item, at least 1.
    cost: u32,
    slot: Slot,
}

/// Queue of one flow; it is in the ring exactly while it holds items.
struct Flow<T> {
    queue: VecDeque<Queued<T>>,
    /// Cost units the flow may still spend in its turn. It stays below the cost of the
    /// oldest item plus a quantum, which `u64` holds for any `u32` cost and quantum.
    deficit: u64,
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
    quantum: u64,
    /// Number of items queued over all flows.
    queued: usize,
    /// Number of [`FlowSender`]s.
    senders: usize,
    /// Number of [`Receiver`]s; the last one to go closes the channel.
    receivers: usize,
    closed: bool,
}

impl<T> Inner<T> {
    fn release(&mut self, id: usize) {
        self.flows[id] = None;
        self.free.push(id);
    }

    /// Returns the number of open flows.
    fn open(&self) -> usize {
        self.flows.len() - self.free.len()
    }

    /// Empties every queue and the ring, frees the flows whose sender is gone, and
    /// returns the items for the caller to drop outside the lock.
    fn drain(&mut self) -> Vec<Queued<T>> {
        let mut queued = Vec::with_capacity(self.queued);

        for id in std::mem::take(&mut self.ring) {
            let flow = self.flows[id].as_mut().expect("flows in the ring are open");
            queued.extend(flow.queue.drain(..));

            if flow.dropped {
                self.release(id);
            }
        }

        self.queued = 0;
        queued
    }

    /// Takes the next item in deficit round robin order.
    fn pick(&mut self) -> Option<(T, Slot)> {
        // Flows passed over since the last item; a whole ring of them means that every
        // flow is short of its oldest item.
        let mut short = 0;

        loop {
            let id = *self.ring.front()?;
            let flow = self.flows[id].as_mut().expect("flows in the ring are open");
            let cost = u64::from(Self::oldest(flow).cost);

            if flow.deficit < cost {
                // Too little left for the oldest item: one more quantum, the others first.
                flow.deficit += self.quantum;
                self.ring.pop_front();
                self.ring.push_back(id);
                short += 1;

                if short == self.ring.len() {
                    self.skip_rounds();
                    short = 0;
                }

                continue;
            }

            flow.deficit -= cost;
            self.queued -= 1;
            let picked = flow.queue.pop_front().map(|queued| (queued.item, queued.slot));

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

    /// Credits every flow of the ring, just passed over in a whole, with the quanta of
    /// the passes that would still go by before one of them has enough for its oldest
    /// item. Whole passes keep the order of the ring, so the next one serves the flow
    /// the skipped ones would have: a heavy item costs one pass instead of one per
    /// quantum.
    fn skip_rounds(&mut self) {
        let quantum = self.quantum;
        let rounds = self
            .ring
            .iter()
            .map(|&id| {
                let flow = self.flows[id].as_ref().expect("flows in the ring are open");
                let shortfall = u64::from(Self::oldest(flow).cost).saturating_sub(flow.deficit);
                shortfall.div_ceil(quantum)
            })
            .min()
            .unwrap_or(0);

        // A flow needing `r` more quanta gets at most `r`, so its deficit stays below
        // its cost plus a quantum, see `Flow::deficit`.
        let credit = rounds * quantum;

        for &id in &self.ring {
            self.flows[id].as_mut().expect("flows in the ring are open").deficit += credit;
        }
    }

    fn oldest(flow: &Flow<T>) -> &Queued<T> {
        flow.queue.front().expect("flows in the ring have queued items")
    }
}
