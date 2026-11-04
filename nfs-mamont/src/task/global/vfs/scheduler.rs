//! Per-connection queues served by deficit round robin, see
//! [`crate::Scheduling::Drr`].
//!
//! Every connection registers a flow ([`Scheduler::register`]) and queues its
//! procedures there. Workers take the next procedure with [`Scheduler::next`]: the
//! flows with queued procedures form a ring, the flow at the head of the ring may spend
//! its deficit on its oldest procedure, and when the deficit is too small the flow is
//! credited one quantum and moved to the tail. A flow whose queue runs empty leaves the
//! ring and loses its deficit, so idle connections never bank credit. The result is
//! max-min fair sharing of the pool between the connections that compete for it: a
//! connection alone gets every worker, backlogged connections get equal cost shares,
//! and a connection that asks for less than its share gets all it asks for.

use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::Notify;

use crate::allocator::Buffer;
use crate::consts::connection::COST_UNIT_BYTES;
use crate::parser::NfsArguments;
use crate::stats;

use super::VfsCommand;

/// Cost of a procedure in scheduler units: one for the request itself plus one per
/// started [`COST_UNIT_BYTES`] of READ or WRITE data.
pub fn cost<B: Buffer>(proc: &NfsArguments<B>) -> u32 {
    let bytes = match proc {
        NfsArguments::Read(args, _) => args.count as usize,
        NfsArguments::Write(args) => args.data.len(),
        _ => 0,
    };
    1 + u32::try_from(bytes.div_ceil(COST_UNIT_BYTES)).unwrap_or(u32::MAX - 1)
}

struct Flow<B: Buffer> {
    queue: VecDeque<VfsCommand<B>>,
    deficit: i64,
    /// Whether the flow is in the ring of flows with queued procedures.
    active: bool,
    /// Set when the connection is gone; the slot is freed once the queue is drained.
    closed: bool,
}

struct Inner<B: Buffer> {
    flows: Vec<Option<Flow<B>>>,
    free: Vec<usize>,
    /// Ring of flows with queued procedures, the head is served next.
    ring: VecDeque<usize>,
    /// Whether the flow at the head of the ring got its quantum for this visit.
    head_credited: bool,
    /// Procedures queued over all flows.
    queued: usize,
}

impl<B: Buffer> Inner<B> {
    fn release(&mut self, id: usize) {
        self.flows[id] = None;
        self.free.push(id);
    }
}

/// What [`Scheduler::pick`] decided for the flow at the head of the ring.
enum Step<B: Buffer> {
    Dispatch(VfsCommand<B>),
    Empty { closed: bool },
    Rotate,
}

/// Queues of all connections and the ring they are served from.
pub struct Scheduler<B: Buffer> {
    inner: Mutex<Inner<B>>,
    /// Wakes a worker when a procedure is queued or the scheduler closes.
    wake: Notify,
    quantum: i64,
    closed: AtomicBool,
}

/// Queue of one connection in the [`Scheduler`]; dropping it unregisters the flow once
/// its queue is drained.
pub struct FlowHandle<B: Buffer> {
    scheduler: Arc<Scheduler<B>>,
    id: usize,
}

impl<B: Buffer> Scheduler<B> {
    /// Creates a scheduler crediting `quantum` cost units per round and flow.
    pub fn new(quantum: NonZeroU32) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner {
                flows: Vec::new(),
                free: Vec::new(),
                ring: VecDeque::new(),
                head_credited: false,
                queued: 0,
            }),
            wake: Notify::new(),
            quantum: i64::from(quantum.get()),
            closed: AtomicBool::new(false),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inner<B>> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Registers the flow of a new connection.
    pub fn register(self: &Arc<Self>) -> FlowHandle<B> {
        let mut guard = self.lock();
        let inner = &mut *guard;
        let flow = Flow { queue: VecDeque::new(), deficit: 0, active: false, closed: false };
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
        drop(guard);

        FlowHandle { scheduler: Arc::clone(self), id }
    }

    /// Stops the scheduler: procedures already queued are still handed out, then
    /// [`Self::next`] returns `None`.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.wake.notify_waiters();
    }

    /// Returns whether [`Self::close`] was called.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Number of procedures queued over all flows.
    #[cfg(test)]
    pub fn queued(&self) -> usize {
        self.lock().queued
    }

    /// Number of registered flows.
    #[cfg(test)]
    pub fn flows(&self) -> usize {
        self.lock().flows.iter().filter(|flow| flow.is_some()).count()
    }

    /// Takes the next procedure to execute, waiting while none is queued.
    ///
    /// # Returns
    ///
    /// `None` once the scheduler is closed and drained.
    pub async fn next(&self) -> Option<VfsCommand<B>> {
        loop {
            // Register for a wake-up before looking, so that an enqueue between the look
            // and the wait is not missed.
            let notified = self.wake.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if let Some(command) = self.pick() {
                return Some(command);
            }
            if self.is_closed() {
                return None;
            }
            notified.await;
        }
    }

    /// One pass of deficit round robin over the ring.
    ///
    /// The flow at the head of the ring is credited one quantum when it arrives there
    /// and served while its deficit covers the cost of its oldest procedure; then it
    /// moves to the tail, keeping the remainder. A flow that runs out of procedures
    /// leaves the ring and loses its deficit.
    fn pick(&self) -> Option<VfsCommand<B>> {
        let mut guard = self.lock();
        let inner = &mut *guard;
        loop {
            let id = *inner.ring.front()?;
            let flow = inner.flows[id].as_mut().expect("flows in the ring are registered");
            if !inner.head_credited {
                flow.deficit += self.quantum;
                inner.head_credited = true;
            }
            let step = match flow.queue.front().map(|command| i64::from(cost(&command.args.proc))) {
                None => {
                    // Served out: leave the ring and lose the deficit.
                    flow.deficit = 0;
                    flow.active = false;
                    Step::Empty { closed: flow.closed }
                }
                Some(cost) if flow.deficit >= cost => {
                    flow.deficit -= cost;
                    Step::Dispatch(flow.queue.pop_front().expect("the front was just seen"))
                }
                Some(_) => Step::Rotate,
            };
            match step {
                Step::Dispatch(command) => {
                    inner.queued -= 1;
                    stats::SCHEDULER_DISPATCHED.fetch_add(1, Ordering::Relaxed);
                    return Some(command);
                }
                Step::Empty { closed } => {
                    inner.ring.pop_front();
                    inner.head_credited = false;
                    if closed {
                        inner.release(id);
                    }
                }
                Step::Rotate => {
                    inner.ring.rotate_left(1);
                    inner.head_credited = false;
                    stats::SCHEDULER_ROTATIONS.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
}

impl<B: Buffer> FlowHandle<B> {
    /// Queues `command` on this flow.
    ///
    /// # Errors
    ///
    /// Gives the command back when the scheduler is closed.
    // The command is returned on purpose, so that the caller can answer it.
    #[allow(clippy::result_large_err)]
    pub fn enqueue(&self, command: VfsCommand<B>) -> Result<(), VfsCommand<B>> {
        if self.scheduler.is_closed() {
            return Err(command);
        }
        {
            let mut guard = self.scheduler.lock();
            let inner = &mut *guard;
            let flow = inner.flows[self.id].as_mut().expect("a handle keeps its flow registered");
            flow.queue.push_back(command);
            if !flow.active {
                flow.active = true;
                inner.ring.push_back(self.id);
            }
            inner.queued += 1;
        }
        self.scheduler.wake.notify_one();

        Ok(())
    }
}

impl<B: Buffer> Drop for FlowHandle<B> {
    fn drop(&mut self) {
        let mut guard = self.scheduler.lock();
        let inner = &mut *guard;
        let Some(flow) = inner.flows[self.id].as_mut() else {
            return;
        };
        if flow.active {
            flow.closed = true;
        } else {
            inner.release(self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU32, NonZeroUsize};
    use std::time::Duration;

    use async_channel::Sender;
    use tokio::time::timeout;

    use super::{cost, Scheduler};
    use crate::allocator::Buffer;
    use crate::parser::tests::allocator::MockBuffer;
    use crate::parser::{NfsArgWrapper, NfsArguments, RpcHeader};
    use crate::rpc::auth::Credential;
    use crate::task::connection::limit::{Budget, Limiter};
    use crate::task::global::vfs::VfsCommand;
    use crate::task::InFlightReply;
    use crate::vfs::file::{BackendId, Handle, PAYLOAD_SIZE};
    use crate::vfs::{get_attr, read, write};
    use std::sync::Arc;

    fn handle() -> Handle {
        Handle::new(BackendId::new(0, 0), [0x01; PAYLOAD_SIZE])
    }

    fn getattr() -> NfsArguments<MockBuffer> {
        NfsArguments::GetAttr(get_attr::Args { file: handle() })
    }

    fn read_of(count: u32) -> NfsArguments<MockBuffer> {
        NfsArguments::Read(read::Args { file: handle(), offset: 0, count }, MockBuffer::empty())
    }

    fn write_of(size: usize) -> NfsArguments<MockBuffer> {
        NfsArguments::Write(write::Args {
            file: handle(),
            offset: 0,
            size: size as u32,
            stable: write::StableHow::Unstable,
            data: MockBuffer::zeroed(size),
        })
    }

    /// Source of commands of one test connection.
    struct Conn {
        budget: Budget,
        _limiter: Arc<Limiter>,
        result_tx: Sender<InFlightReply<MockBuffer>>,
    }

    impl Conn {
        fn new() -> Self {
            let (budget, limiter) = Budget::new(NonZeroUsize::new(1 << 20).unwrap());
            let (result_tx, _result_rx) = async_channel::unbounded();
            std::mem::forget(_result_rx);
            Self { budget, _limiter: limiter, result_tx }
        }

        async fn command(
            &mut self,
            xid: u32,
            proc: NfsArguments<MockBuffer>,
        ) -> VfsCommand<MockBuffer> {
            let token = self.budget.acquire().await.unwrap();
            VfsCommand {
                args: NfsArgWrapper {
                    header: RpcHeader { xid, cred: Credential::None },
                    proc: Box::new(proc),
                },
                result_tx: self.result_tx.clone(),
                token,
                pending: None,
            }
        }
    }

    fn quantum(value: u32) -> NonZeroU32 {
        NonZeroU32::new(value).unwrap()
    }

    #[test]
    fn cost_grows_with_the_data() {
        assert_eq!(cost(&getattr()), 1);
        assert_eq!(cost(&read_of(4096)), 2);
        assert_eq!(cost(&read_of(64 * 1024)), 2);
        assert_eq!(cost(&read_of(1 << 20)), 17);
        assert_eq!(cost(&write_of(64 * 1024)), 2);
        assert_eq!(cost(&write_of(0)), 1);
    }

    #[tokio::test]
    async fn one_flow_is_served_in_order() {
        let scheduler = Scheduler::new(quantum(32));
        let flow = scheduler.register();
        let mut conn = Conn::new();
        for xid in 1..=5 {
            flow.enqueue(conn.command(xid, getattr()).await).ok().expect("the scheduler is open");
        }
        assert_eq!(scheduler.queued(), 5);

        for xid in 1..=5 {
            let command = scheduler.next().await.unwrap();
            assert_eq!(command.args.header.xid, xid);
        }
        assert_eq!(scheduler.queued(), 0);
    }

    #[tokio::test]
    async fn backlogged_flows_get_equal_cost_shares() {
        let scheduler = Scheduler::new(quantum(32));
        let (flow_a, flow_b) = (scheduler.register(), scheduler.register());
        let (mut conn_a, mut conn_b) = (Conn::new(), Conn::new());
        for i in 0..200 {
            flow_a
                .enqueue(conn_a.command(1000 + i, getattr()).await)
                .ok()
                .expect("the scheduler is open");
            flow_b
                .enqueue(conn_b.command(2000 + i, read_of(1 << 20)).await)
                .ok()
                .expect("the scheduler is open");
        }

        let (mut cost_a, mut cost_b) = (0u32, 0u32);
        for _ in 0..100 {
            let command = scheduler.next().await.unwrap();
            if command.args.header.xid < 2000 {
                cost_a += cost(&command.args.proc);
            } else {
                cost_b += cost(&command.args.proc);
            }
        }
        assert!(cost_a > 0 && cost_b > 0, "both flows were served: {cost_a} vs {cost_b}");
        assert!(
            cost_a.abs_diff(cost_b) <= 2 * 32,
            "cost shares differ by more than two quanta: {cost_a} vs {cost_b}"
        );
    }

    #[tokio::test]
    async fn a_light_flow_waits_at_most_one_round() {
        let scheduler = Scheduler::new(quantum(32));
        let (heavy, light) = (scheduler.register(), scheduler.register());
        let (mut conn_h, mut conn_l) = (Conn::new(), Conn::new());
        for i in 0..500 {
            heavy
                .enqueue(conn_h.command(1000 + i, getattr()).await)
                .ok()
                .expect("the scheduler is open");
        }
        // The heavy flow is being served.
        for _ in 0..10 {
            scheduler.next().await.unwrap();
        }
        light.enqueue(conn_l.command(1, getattr()).await).ok().expect("the scheduler is open");

        let mut dispatched_before_light = 0;
        loop {
            let command = scheduler.next().await.unwrap();
            if command.args.header.xid == 1 {
                break;
            }
            dispatched_before_light += 1;
            assert!(dispatched_before_light <= 32, "light flow waited more than one quantum");
        }
    }

    #[tokio::test]
    async fn an_idle_flow_banks_no_credit() {
        let scheduler = Scheduler::new(quantum(4));
        let (flow_a, flow_b) = (scheduler.register(), scheduler.register());
        let (mut conn_a, mut conn_b) = (Conn::new(), Conn::new());
        // A is served alone for a while: many rotations, but its deficit is reset
        // when its queue empties.
        for i in 0..40 {
            flow_a
                .enqueue(conn_a.command(1000 + i, getattr()).await)
                .ok()
                .expect("the scheduler is open");
        }
        for _ in 0..40 {
            scheduler.next().await.unwrap();
        }
        // Now both compete: B must not wait for A to spend banked credit.
        for i in 0..20 {
            flow_a
                .enqueue(conn_a.command(3000 + i, getattr()).await)
                .ok()
                .expect("the scheduler is open");
            flow_b
                .enqueue(conn_b.command(2000 + i, getattr()).await)
                .ok()
                .expect("the scheduler is open");
        }
        let mut first_b = None;
        for n in 0..40 {
            let command = scheduler.next().await.unwrap();
            if command.args.header.xid >= 2000 && command.args.header.xid < 3000 {
                first_b = Some(n);
                break;
            }
        }
        assert!(first_b.unwrap() <= 4, "B waited {:?} dispatches", first_b);
    }

    #[tokio::test]
    async fn next_waits_for_an_enqueue() {
        let scheduler = Scheduler::new(quantum(32));
        let flow = scheduler.register();
        let mut conn = Conn::new();
        let command = conn.command(7, getattr()).await;
        let producer = tokio::spawn({
            let scheduler = Arc::clone(&scheduler);
            async move {
                tokio::time::sleep(Duration::from_millis(20)).await;
                flow.enqueue(command).ok().expect("the scheduler is open");
                drop(scheduler);
            }
        });
        let command = timeout(Duration::from_secs(5), scheduler.next()).await.unwrap().unwrap();
        assert_eq!(command.args.header.xid, 7);
        producer.await.unwrap();
    }

    #[tokio::test]
    async fn closing_drains_then_stops() {
        let scheduler = Scheduler::new(quantum(32));
        let flow = scheduler.register();
        let mut conn = Conn::new();
        for xid in 1..=3 {
            flow.enqueue(conn.command(xid, getattr()).await).ok().expect("the scheduler is open");
        }
        scheduler.close();
        assert!(flow.enqueue(conn.command(4, getattr()).await).is_err());
        for xid in 1..=3 {
            assert_eq!(scheduler.next().await.unwrap().args.header.xid, xid);
        }
        assert!(scheduler.next().await.is_none());
    }

    #[tokio::test]
    async fn a_dropped_flow_is_freed_once_drained() {
        let scheduler = Scheduler::new(quantum(32));
        let flow = scheduler.register();
        let mut conn = Conn::new();
        flow.enqueue(conn.command(1, getattr()).await).ok().expect("the scheduler is open");
        flow.enqueue(conn.command(2, getattr()).await).ok().expect("the scheduler is open");
        drop(flow);
        assert_eq!(scheduler.flows(), 1, "a flow with queued work stays registered");

        scheduler.next().await.unwrap();
        scheduler.next().await.unwrap();
        let _idle = scheduler.register();
        drop(_idle);
        assert_eq!(scheduler.flows(), 1, "drained flow still in the ring until visited");
        // The next pass over the ring sees the empty closed flow and frees it.
        let producer_flow = scheduler.register();
        producer_flow
            .enqueue(conn.command(3, getattr()).await)
            .ok()
            .expect("the scheduler is open");
        scheduler.next().await.unwrap();
        assert_eq!(scheduler.flows(), 1);
    }
}
