//! Queues of the VFS pool, one per connection, served by deficit round robin.
//!
//! The flows of the connections that have procedures queued form a ring. A worker takes
//! the oldest procedure of the flow at the head while the flow's deficit covers its cost;
//! otherwise the flow is credited one more quantum and moves to the tail. A flow that
//! runs empty leaves the ring and loses its deficit, so an idle connection banks no
//! credit. A connection alone gets every worker, backlogged connections get equal shares
//! of the cost, and a light connection waits for at most one quantum of each heavy one.

use std::collections::VecDeque;
use std::pin::pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::Notify;

use crate::allocator::Buffer;
use crate::parser::NfsArguments;

use super::VfsCommand;

/// Bytes of READ or WRITE data that cost as much as one procedure.
const COST_UNIT: u32 = 64 * 1024;

/// Cost units a flow may spend per turn.
///
/// A light connection next to a heavy one on a 1 ms backend gets a p50 of 1.4 ms with 8
/// and 1.8 ms with 32, at no cost in throughput (measured in #456); a 1 MiB READ, 17
/// units, is served over three turns.
pub const QUANTUM: u32 = 8;

/// Returns the cost of `proc`: one unit, plus one per started [`COST_UNIT`] of READ or
/// WRITE data.
pub fn cost<B: Buffer>(proc: &NfsArguments<B>) -> u32 {
    let bytes = match proc {
        NfsArguments::Read(args, _) => args.count,
        NfsArguments::Write(args) => args.size,
        _ => 0,
    };
    1 + bytes.div_ceil(COST_UNIT)
}

/// Queue of one connection; it is in the ring exactly while it holds procedures.
struct Flow<B: Buffer> {
    queue: VecDeque<VfsCommand<B>>,
    /// Cost units the flow may still spend in its turn.
    deficit: u32,
    /// Whether its connection is gone; the flow is freed once its queue is drained.
    dropped: bool,
}

struct Inner<B: Buffer> {
    /// Flows by id; `None` marks an id that is free for the next connection.
    flows: Vec<Option<Flow<B>>>,
    free: Vec<usize>,
    /// Flows with queued procedures; the head is served next.
    ring: VecDeque<usize>,
    closed: bool,
}

impl<B: Buffer> Inner<B> {
    /// Takes the next procedure in deficit round robin order.
    fn pick(&mut self) -> Option<VfsCommand<B>> {
        loop {
            let id = *self.ring.front()?;
            let flow = self.flows[id].as_mut().expect("flows in the ring are registered");
            let oldest = flow.queue.front().expect("flows in the ring have queued procedures");
            let cost = cost(&oldest.args.proc);
            if flow.deficit < cost {
                // Too little left for the oldest procedure: one more quantum, the others first.
                flow.deficit += QUANTUM;
                self.ring.rotate_left(1);
                continue;
            }
            flow.deficit -= cost;
            let command = flow.queue.pop_front();
            if flow.queue.is_empty() {
                // Served out: leave the ring.
                let dropped = flow.dropped;
                self.ring.pop_front();
                if dropped {
                    self.release(id);
                }
            }
            return command;
        }
    }

    fn release(&mut self, id: usize) {
        self.flows[id] = None;
        self.free.push(id);
    }
}

/// Queues of all connections and the ring they are served from.
pub struct Scheduler<B: Buffer> {
    inner: Mutex<Inner<B>>,
    /// Wakes a worker when a procedure is queued or the scheduler closes.
    wake: Notify,
}

impl<B: Buffer> Scheduler<B> {
    /// Creates a scheduler with no flows.
    pub fn new() -> Arc<Self> {
        let inner =
            Inner { flows: Vec::new(), free: Vec::new(), ring: VecDeque::new(), closed: false };
        Arc::new(Self { inner: Mutex::new(inner), wake: Notify::new() })
    }

    fn lock(&self) -> MutexGuard<'_, Inner<B>> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers the flow of a new connection.
    pub fn register(self: &Arc<Self>) -> FlowHandle<B> {
        let mut inner = self.lock();
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

        FlowHandle { scheduler: Arc::clone(self), id }
    }

    /// Stops taking procedures: those already queued are still handed out, then
    /// [`Self::next`] returns `None`.
    pub fn close(&self) {
        self.lock().closed = true;
        self.wake.notify_waiters();
    }

    /// Takes the next procedure to execute, waiting while none is queued.
    ///
    /// # Returns
    ///
    /// `None` once the scheduler is closed and drained.
    pub async fn next(&self) -> Option<VfsCommand<B>> {
        loop {
            // Registered before looking, so that a procedure queued in between wakes us.
            let mut notified = pin!(self.wake.notified());
            notified.as_mut().enable();
            {
                let mut inner = self.lock();
                if let Some(command) = inner.pick() {
                    return Some(command);
                }
                if inner.closed {
                    return None;
                }
            }
            notified.await;
        }
    }

    /// Returns the number of procedures queued over all flows.
    #[cfg(test)]
    pub fn queued(&self) -> usize {
        self.lock().flows.iter().flatten().map(|flow| flow.queue.len()).sum()
    }

    /// Returns the number of registered flows, including dropped ones not yet drained.
    #[cfg(test)]
    pub fn flows(&self) -> usize {
        self.lock().flows.iter().flatten().count()
    }

    /// Returns the number of flows whose connection still holds its handle.
    #[cfg(test)]
    pub fn connections(&self) -> usize {
        self.lock().flows.iter().flatten().filter(|flow| !flow.dropped).count()
    }
}

/// Queue of one connection in the [`Scheduler`]; dropping it frees the queue once its
/// procedures are handed out.
pub struct FlowHandle<B: Buffer> {
    scheduler: Arc<Scheduler<B>>,
    id: usize,
}

impl<B: Buffer> FlowHandle<B> {
    /// Queues `command` on this flow.
    ///
    /// # Errors
    ///
    /// Gives the command back when the scheduler is closed.
    pub fn enqueue(&self, command: VfsCommand<B>) -> Result<(), VfsCommand<B>> {
        {
            let mut guard = self.scheduler.lock();
            let inner = &mut *guard;
            if inner.closed {
                return Err(command);
            }
            let flow = inner.flows[self.id].as_mut().expect("a handle keeps its flow registered");
            if flow.queue.is_empty() {
                // A flow joining the ring starts a fresh turn: what it left unspent
                // before is lost, so an idle connection banks no credit.
                flow.deficit = QUANTUM;
                inner.ring.push_back(self.id);
            }
            flow.queue.push_back(command);
        }
        self.scheduler.wake.notify_one();

        Ok(())
    }
}

impl<B: Buffer> Drop for FlowHandle<B> {
    fn drop(&mut self) {
        let mut inner = self.scheduler.lock();
        let flow = inner.flows[self.id].as_mut().expect("a handle keeps its flow registered");
        if flow.queue.is_empty() {
            inner.release(self.id);
        } else {
            flow.dropped = true;
        }
    }
}
