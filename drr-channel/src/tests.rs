use std::collections::VecDeque;
use std::num::{NonZeroU32, NonZeroUsize};
use std::pin::pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;

use crate::{Cost, Receiver, Sender};

/// How long the tests wait to see that something does not happen.
const WAIT: Duration = Duration::from_millis(50);
/// A capacity the tests never reach.
const ROOMY: NonZeroUsize = NonZeroUsize::MAX;
/// Cost units a flow may spend per turn.
const QUANTUM: u32 = 4;
/// Cost of an item that takes several turns.
const HEAVY: u32 = 17;

struct Item {
    id: u32,
    cost: u32,
}

impl Item {
    fn new(id: u32, cost: u32) -> Self {
        Self { id, cost }
    }
}

impl Cost for Item {
    fn cost(&self) -> u32 {
        self.cost
    }
}

/// Creates a channel of `capacity` items with a quantum of [`QUANTUM`].
fn channel(capacity: NonZeroUsize) -> (Sender<Item>, Receiver<Item>) {
    crate::channel(capacity, NonZeroU32::new(QUANTUM).unwrap())
}

/// Takes the next item, which must be queued already; returns its id.
async fn next_id(receiver: &Receiver<Item>) -> u32 {
    let (item, _slot) = timeout(Duration::from_secs(5), receiver.recv()).await.unwrap().unwrap();
    item.id
}

#[tokio::test]
async fn one_flow_is_served_in_order() {
    let (sender, receiver) = channel(ROOMY);
    let mut flow = sender.flow();
    for id in 1..=20 {
        assert!(flow.send(Item::new(id, HEAVY)).await.is_ok());
    }

    for id in 1..=20 {
        assert_eq!(next_id(&receiver).await, id);
    }
}

#[tokio::test]
async fn backlogged_flows_get_equal_cost_shares() {
    let (sender, receiver) = channel(ROOMY);
    let (mut light, mut heavy) = (sender.flow(), sender.flow());
    for i in 0..500 {
        assert!(light.send(Item::new(1000 + i, 1)).await.is_ok());
        assert!(heavy.send(Item::new(2000 + i, HEAVY)).await.is_ok());
    }

    let (mut light_cost, mut heavy_cost) = (0, 0);
    for _ in 0..200 {
        let (item, _slot) = receiver.recv().await.unwrap();
        if item.id < 2000 {
            light_cost += item.cost;
        } else {
            heavy_cost += item.cost;
        }
    }
    // The heavy flow spends its quanta in steps of one item.
    let bound = QUANTUM + HEAVY;
    assert!(light_cost.abs_diff(heavy_cost) <= bound, "{light_cost} against {heavy_cost}");
}

#[tokio::test]
async fn a_light_flow_waits_for_at_most_one_quantum() {
    let (sender, receiver) = channel(ROOMY);
    let (mut heavy, mut light) = (sender.flow(), sender.flow());
    for i in 0..100 {
        assert!(heavy.send(Item::new(1000 + i, 1)).await.is_ok());
    }
    // The heavy flow is in the middle of its turn when the light one arrives.
    next_id(&receiver).await;
    assert!(light.send(Item::new(1, 1)).await.is_ok());

    let mut before = 0;
    while next_id(&receiver).await != 1 {
        before += 1;
    }
    assert!(before < QUANTUM, "the light flow waited for {before} items");
}

#[tokio::test]
async fn an_idle_flow_banks_no_credit() {
    let (sender, receiver) = channel(ROOMY);
    let (mut first, mut second) = (sender.flow(), sender.flow());
    // The first flow is served alone, then runs empty.
    for i in 0..40 {
        assert!(first.send(Item::new(1000 + i, 1)).await.is_ok());
    }
    for _ in 0..40 {
        next_id(&receiver).await;
    }
    for i in 0..20 {
        assert!(first.send(Item::new(3000 + i, 1)).await.is_ok());
        assert!(second.send(Item::new(2000 + i, 1)).await.is_ok());
    }

    let mut before = 0;
    while !(2000..3000).contains(&next_id(&receiver).await) {
        before += 1;
    }
    assert!(before <= QUANTUM, "the second flow waited for {before} items");
}

#[tokio::test]
async fn recv_waits_for_an_item() {
    let (sender, receiver) = channel(ROOMY);
    let mut flow = sender.flow();

    let producer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(flow.send(Item::new(7, 1)).await.is_ok());
    });
    assert_eq!(next_id(&receiver).await, 7);
    producer.await.unwrap();
}

#[tokio::test]
async fn dropping_the_sender_hands_out_the_queued_items_then_stops() {
    let (sender, receiver) = channel(ROOMY);
    let mut flow = sender.flow();
    for id in 1..=3 {
        assert!(flow.send(Item::new(id, 1)).await.is_ok());
    }

    drop(sender);
    assert!(flow.send(Item::new(4, 1)).await.is_err());
    for id in 1..=3 {
        assert_eq!(next_id(&receiver).await, id);
    }
    assert!(receiver.recv().await.is_none());
}

#[tokio::test]
async fn a_dropped_flow_is_freed_once_drained() {
    let (sender, receiver) = channel(ROOMY);
    let mut flow = sender.flow();
    for id in 1..=2 {
        assert!(flow.send(Item::new(id, 1)).await.is_ok());
    }

    drop(flow);
    assert_eq!(receiver.flows(), 1, "a flow with queued items stays open");
    assert_eq!(next_id(&receiver).await, 1);
    assert_eq!(receiver.flows(), 1);
    assert_eq!(next_id(&receiver).await, 2);
    assert_eq!(receiver.flows(), 0);

    drop(sender.flow());
    assert_eq!(receiver.flows(), 0, "an idle flow is freed right away");
}

#[tokio::test]
async fn an_item_counts_against_the_capacity_until_its_slot_is_dropped() {
    let (sender, receiver) = channel(NonZeroUsize::new(2).unwrap());
    let mut flow = sender.flow();
    assert!(flow.send(Item::new(1, 1)).await.is_ok());
    assert!(flow.send(Item::new(2, 1)).await.is_ok());
    assert!(timeout(WAIT, flow.send(Item::new(3, 1))).await.is_err(), "sent past the capacity");

    // Taken but still held: the capacity stays full.
    let first = receiver.recv().await.unwrap();
    assert!(timeout(WAIT, flow.send(Item::new(3, 1))).await.is_err(), "a held item lost its slot");

    drop(first);
    assert!(timeout(WAIT, flow.send(Item::new(3, 1))).await.unwrap().is_ok());
}

#[tokio::test]
async fn closing_fails_the_senders_waiting_for_a_slot() {
    let (sender, _receiver) = channel(NonZeroUsize::new(1).unwrap());
    let mut flow = sender.flow();
    assert!(flow.send(Item::new(1, 1)).await.is_ok());
    {
        let mut waiting = pin!(flow.send(Item::new(2, 1)));
        assert!(timeout(WAIT, &mut waiting).await.is_err(), "sent past the capacity");

        drop(sender);
        let failed = timeout(WAIT, waiting).await.unwrap();
        assert!(failed.is_err(), "a waiting sender outlived the channel");
    }
    assert!(timeout(WAIT, flow.send(Item::new(3, 1))).await.unwrap().is_err());
}

/// Item that counts the calls of [`Cost::cost`] and tells by its counter whether it is
/// still alive.
struct Tracked {
    calls: Arc<AtomicUsize>,
    cost: u32,
}

impl Cost for Tracked {
    fn cost(&self) -> u32 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.cost
    }
}

#[tokio::test]
async fn dropping_the_last_receiver_closes_the_channel_and_drops_the_queued_items() {
    let (sender, receiver) = crate::channel(NonZeroUsize::new(2).unwrap(), NonZeroU32::MIN);
    let (mut flow, mut gone) = (sender.flow(), sender.flow());
    let calls = Arc::new(AtomicUsize::new(0));
    let tracked = || Tracked { calls: Arc::clone(&calls), cost: 1 };
    assert!(flow.send(tracked()).await.is_ok());
    assert!(gone.send(tracked()).await.is_ok());
    drop(gone);
    {
        let mut waiting = pin!(flow.send(tracked()));
        assert!(timeout(WAIT, &mut waiting).await.is_err(), "sent past the capacity");

        drop(receiver.clone());
        assert!(timeout(WAIT, &mut waiting).await.is_err(), "a clone closed the channel");

        drop(receiver);
        let failed = timeout(WAIT, waiting).await.unwrap();
        assert!(failed.is_err(), "a waiting sender outlived the channel");
    }
    assert!(flow.send(tracked()).await.is_err());
    assert_eq!(Arc::strong_count(&calls), 1, "queued items were not dropped");
    let open = || sender.shared.lock().flows.iter().flatten().count();
    assert_eq!(open(), 1, "the drained flow of a dropped sender was kept");
    drop(flow);
    assert_eq!(open(), 0);
}

#[tokio::test]
async fn cost_is_asked_once_per_item() {
    let (sender, receiver) = crate::channel(ROOMY, NonZeroU32::MIN);
    let (mut heavy, mut light) = (sender.flow(), sender.flow());
    let calls = Arc::new(AtomicUsize::new(0));
    // The heavy item goes round the ring many times while the light ones are served.
    assert!(heavy.send(Tracked { calls: Arc::clone(&calls), cost: HEAVY }).await.is_ok());
    for _ in 0..HEAVY {
        assert!(light.send(Tracked { calls: Arc::clone(&calls), cost: 1 }).await.is_ok());
    }

    for _ in 0..=HEAVY {
        receiver.recv().await.unwrap();
    }
    assert_eq!(calls.load(Ordering::Relaxed), 1 + HEAVY as usize);
}

#[tokio::test]
async fn a_zero_cost_counts_as_one() {
    let (sender, receiver) = channel(ROOMY);
    let (mut free, mut other) = (sender.flow(), sender.flow());
    for i in 0..100 {
        assert!(free.send(Item::new(1000 + i, 0)).await.is_ok());
    }
    assert!(other.send(Item::new(1, 1)).await.is_ok());

    let mut before = 0;
    while next_id(&receiver).await != 1 {
        before += 1;
    }
    assert!(before <= QUANTUM, "the other flow waited for {before} items");
}

#[tokio::test]
async fn the_largest_costs_and_quantum_do_not_overflow() {
    // The deficit of a flow short of the largest cost, credited a quantum, is past `u32`.
    let quantum = NonZeroU32::new(u32::MAX / 2 + 1).unwrap();
    let (sender, receiver) = crate::channel(ROOMY, quantum);
    let mut flow = sender.flow();
    for id in 1..=3 {
        assert!(flow.send(Item::new(id, u32::MAX)).await.is_ok());
    }

    for id in 1..=3 {
        assert_eq!(next_id(&receiver).await, id);
    }
}

#[tokio::test]
async fn a_heavy_item_costs_one_pass_over_the_ring() {
    // Going round a quantum at a time, this would take billions of turns.
    let (sender, receiver) = crate::channel(ROOMY, NonZeroU32::MIN);
    let (mut first, mut second) = (sender.flow(), sender.flow());
    assert!(first.send(Item::new(1, u32::MAX)).await.is_ok());
    assert!(second.send(Item::new(2, u32::MAX - 1)).await.is_ok());
    assert!(first.send(Item::new(3, u32::MAX)).await.is_ok());

    assert_eq!(next_id(&receiver).await, 2, "the flow short by less goes first");
    assert_eq!(next_id(&receiver).await, 1);
    assert_eq!(next_id(&receiver).await, 3);
}

/// Deficit round robin a quantum at a time, as the channel did before skipping rounds.
struct Model {
    quantum: u64,
    queues: Vec<VecDeque<(u32, u32)>>,
    deficits: Vec<u64>,
    ring: VecDeque<usize>,
}

impl Model {
    fn send(&mut self, flow: usize, id: u32, cost: u32) {
        if self.queues[flow].is_empty() {
            self.deficits[flow] = self.quantum;
            self.ring.push_back(flow);
        }
        self.queues[flow].push_back((id, cost.max(1)));
    }

    fn recv(&mut self) -> Option<u32> {
        loop {
            let flow = *self.ring.front()?;
            let (id, cost) = self.queues[flow][0];
            if self.deficits[flow] < u64::from(cost) {
                self.deficits[flow] += self.quantum;
                self.ring.rotate_left(1);
                continue;
            }
            self.deficits[flow] -= u64::from(cost);
            self.queues[flow].pop_front();
            if self.queues[flow].is_empty() {
                self.ring.pop_front();
            }
            return Some(id);
        }
    }
}

/// Pseudo-random numbers, the same on every run.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, bound: u32) -> u32 {
        self.0 =
            self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % u64::from(bound)) as u32
    }
}

#[tokio::test]
async fn skipping_rounds_keeps_the_order_of_deficit_round_robin() {
    const FLOWS: usize = 5;
    let mut random = Lcg(7);
    for quantum in [1, 3, 8] {
        let (sender, receiver) = crate::channel(ROOMY, NonZeroU32::new(quantum).unwrap());
        let mut flows: Vec<_> = (0..FLOWS).map(|_| sender.flow()).collect();
        let mut model = Model {
            quantum: u64::from(quantum),
            queues: vec![VecDeque::new(); FLOWS],
            deficits: vec![0; FLOWS],
            ring: VecDeque::new(),
        };
        let mut order = (Vec::new(), Vec::new());
        for id in 0..5000 {
            // Sends and receives interleaved, so that flows leave the ring and join it.
            if random.below(2) == 0 {
                let (flow, cost) = (random.below(FLOWS as u32) as usize, random.below(40));
                assert!(flows[flow].send(Item::new(id, cost)).await.is_ok());
                model.send(flow, id, cost);
            } else if let Some(expected) = model.recv() {
                order.0.push(next_id(&receiver).await);
                order.1.push(expected);
            }
        }
        while let Some(expected) = model.recv() {
            order.0.push(next_id(&receiver).await);
            order.1.push(expected);
        }
        assert_eq!(order.0, order.1, "quantum {quantum}");
    }
}

#[tokio::test]
async fn waiting_flows_take_the_slots_in_turn() {
    const ITEMS: u32 = 10;
    let (sender, receiver) = channel(NonZeroUsize::MIN);
    let producers: Vec<_> = [1000, 2000]
        .into_iter()
        .map(|base| {
            let mut flow = sender.flow();
            tokio::spawn(async move {
                for i in 0..ITEMS {
                    assert!(flow.send(Item::new(base + i, 1)).await.is_ok());
                }
            })
        })
        .collect();

    let mut flows = Vec::new();
    for _ in 0..2 * ITEMS {
        flows.push(next_id(&receiver).await / 1000);
    }
    for producer in producers {
        producer.await.unwrap();
    }
    // One flow may get a second slot before the other one asks for its first.
    let runs = flows.windows(3).filter(|three| three[0] == three[1] && three[1] == three[2]);
    assert_eq!(runs.count(), 0, "the flows were served {flows:?}");
}

#[tokio::test]
async fn the_counters_follow_the_flows_and_items() {
    let (sender, receiver) = channel(ROOMY);
    let counters =
        |receiver: &Receiver<Item>| (receiver.queued(), receiver.flows(), receiver.senders());
    let (mut first, mut second) = (sender.flow(), sender.flow());
    assert_eq!(counters(&receiver), (0, 2, 2));

    for id in 1..=3 {
        assert!(first.send(Item::new(id, 1)).await.is_ok());
    }
    assert!(second.send(Item::new(4, 1)).await.is_ok());
    assert_eq!(counters(&receiver), (4, 2, 2));

    drop(first);
    assert_eq!(counters(&receiver), (4, 2, 1), "a dropped flow stays until drained");
    for _ in 0..4 {
        next_id(&receiver).await;
    }
    assert_eq!(counters(&receiver), (0, 1, 1));

    drop(second);
    assert_eq!(counters(&receiver), (0, 0, 0));
}
