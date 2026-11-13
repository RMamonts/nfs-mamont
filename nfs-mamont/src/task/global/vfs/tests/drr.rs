//! Tests of the deficit round robin channel, [`crate::task::global::vfs::drr`].

use std::num::NonZeroUsize;
use std::time::Duration;

use tokio::time::timeout;

use crate::task::global::vfs::drr::{channel, Cost, Receiver, QUANTUM};

/// How long the tests wait to see that something does not happen.
const WAIT: Duration = Duration::from_millis(50);
/// Cost of a 1 MiB READ.
const HEAVY: u32 = 17;

struct Item {
    id: u32,
    cost: u32,
}

impl Cost for Item {
    fn cost(&self) -> u32 {
        self.cost
    }
}

fn item(id: u32, cost: u32) -> Item {
    Item { id, cost }
}

/// Takes the next item, which must be queued already; returns its id.
async fn next_id(receiver: &Receiver<Item>) -> u32 {
    let (item, _slot) = timeout(Duration::from_secs(5), receiver.recv()).await.unwrap().unwrap();
    item.id
}

#[tokio::test]
async fn one_flow_is_served_in_order() {
    let (sender, receiver) = channel(None);
    let flow = sender.flow();
    for id in 1..=20 {
        assert!(flow.send(item(id, HEAVY)).await.is_ok());
    }

    for id in 1..=20 {
        assert_eq!(next_id(&receiver).await, id);
    }
}

#[tokio::test]
async fn backlogged_flows_get_equal_cost_shares() {
    let (sender, receiver) = channel(None);
    let (light, heavy) = (sender.flow(), sender.flow());
    for i in 0..500 {
        assert!(light.send(item(1000 + i, 1)).await.is_ok());
        assert!(heavy.send(item(2000 + i, HEAVY)).await.is_ok());
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
    let (sender, receiver) = channel(None);
    let (heavy, light) = (sender.flow(), sender.flow());
    for i in 0..100 {
        assert!(heavy.send(item(1000 + i, 1)).await.is_ok());
    }
    // The heavy flow is in the middle of its turn when the light one arrives.
    next_id(&receiver).await;
    assert!(light.send(item(1, 1)).await.is_ok());

    let mut before = 0;
    while next_id(&receiver).await != 1 {
        before += 1;
    }
    assert!(before < QUANTUM, "the light flow waited for {before} items");
}

#[tokio::test]
async fn an_idle_flow_banks_no_credit() {
    let (sender, receiver) = channel(None);
    let (first, second) = (sender.flow(), sender.flow());
    // The first flow is served alone, then runs empty.
    for i in 0..40 {
        assert!(first.send(item(1000 + i, 1)).await.is_ok());
    }
    for _ in 0..40 {
        next_id(&receiver).await;
    }
    for i in 0..20 {
        assert!(first.send(item(3000 + i, 1)).await.is_ok());
        assert!(second.send(item(2000 + i, 1)).await.is_ok());
    }

    let mut before = 0;
    while !(2000..3000).contains(&next_id(&receiver).await) {
        before += 1;
    }
    assert!(before <= QUANTUM, "the second flow waited for {before} items");
}

#[tokio::test]
async fn recv_waits_for_an_item() {
    let (sender, receiver) = channel(None);
    let flow = sender.flow();

    let producer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(flow.send(item(7, 1)).await.is_ok());
    });
    assert_eq!(next_id(&receiver).await, 7);
    producer.await.unwrap();
}

#[tokio::test]
async fn dropping_the_sender_hands_out_the_queued_items_then_stops() {
    let (sender, receiver) = channel(None);
    let flow = sender.flow();
    for id in 1..=3 {
        assert!(flow.send(item(id, 1)).await.is_ok());
    }

    drop(sender);
    assert!(flow.send(item(4, 1)).await.is_err());
    for id in 1..=3 {
        assert_eq!(next_id(&receiver).await, id);
    }
    assert!(receiver.recv().await.is_none());
}

#[tokio::test]
async fn a_dropped_flow_is_freed_once_drained() {
    let (sender, receiver) = channel(None);
    let flow = sender.flow();
    for id in 1..=2 {
        assert!(flow.send(item(id, 1)).await.is_ok());
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
    let (sender, receiver) = channel(Some(NonZeroUsize::new(2).unwrap()));
    let flow = sender.flow();
    assert!(flow.send(item(1, 1)).await.is_ok());
    assert!(flow.send(item(2, 1)).await.is_ok());
    assert!(timeout(WAIT, flow.send(item(3, 1))).await.is_err(), "sent past the capacity");

    // Taken but still held: the capacity stays full.
    let first = receiver.recv().await.unwrap();
    assert!(timeout(WAIT, flow.send(item(3, 1))).await.is_err(), "a held item lost its slot");

    drop(first);
    assert!(timeout(WAIT, flow.send(item(3, 1))).await.unwrap().is_ok());
}
