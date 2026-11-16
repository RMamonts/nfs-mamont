//! Tests of the deficit round robin of [`crate::task::global::vfs::scheduler`].

use std::num::NonZeroUsize;
use std::time::Duration;

use async_channel::Receiver;
use tokio::time::timeout;

use crate::parser::{NfsArgWrapper, NfsArguments, RpcHeader};
use crate::rpc::auth::Credential;
use crate::task::connection::limit::{Budget, InFlightReply};
use crate::task::global::vfs::scheduler::{cost, Scheduler, QUANTUM};
use crate::task::global::vfs::VfsCommand;
use crate::vfs::nfsv3::{read, write};

use super::{handle, MockBuffer};

fn getattr() -> NfsArguments<MockBuffer> {
    NfsArguments::GetAttr(crate::vfs::nfsv3::get_attr::Args { file: handle(0) })
}

fn read_of(count: u32) -> NfsArguments<MockBuffer> {
    NfsArguments::Read(read::Args { file: handle(0), offset: 0, count }, MockBuffer)
}

fn write_of(size: u32) -> NfsArguments<MockBuffer> {
    let stable = write::StableHow::Unstable;
    NfsArguments::Write(write::Args { file: handle(0), offset: 0, size, stable, data: MockBuffer })
}

/// Makes the commands of one test connection.
struct Connection {
    budget: Budget<MockBuffer>,
    // Kept open so that replies could still be queued.
    _replies: Receiver<InFlightReply<MockBuffer>>,
}

impl Connection {
    fn new() -> Self {
        let (tx, replies) = async_channel::unbounded();
        let budget = Budget::new(NonZeroUsize::new(1 << 20).unwrap(), None, None, tx);
        Self { budget, _replies: replies }
    }

    async fn command(
        &mut self,
        xid: u32,
        proc: NfsArguments<MockBuffer>,
    ) -> VfsCommand<MockBuffer> {
        let header = RpcHeader { xid, cred: Credential::None };
        let args = NfsArgWrapper { header, proc: Box::new(proc) };
        VfsCommand { args, result_tx: self.budget.acquire().await, pending: None }
    }
}

/// Takes the next command, which must be queued already; returns its xid.
async fn next_xid(scheduler: &Scheduler<MockBuffer>) -> u32 {
    let command = timeout(Duration::from_secs(5), scheduler.next()).await.unwrap().unwrap();
    command.args.header.xid
}

#[test]
fn cost_grows_with_the_data() {
    assert_eq!(cost(&getattr()), 1);
    assert_eq!(cost(&read_of(0)), 1);
    assert_eq!(cost(&read_of(4096)), 2);
    assert_eq!(cost(&read_of(64 * 1024)), 2);
    assert_eq!(cost(&read_of(64 * 1024 + 1)), 3);
    assert_eq!(cost(&read_of(1 << 20)), 17);
    assert_eq!(cost(&write_of(64 * 1024)), 2);
}

#[tokio::test]
async fn one_flow_is_served_in_order() {
    let scheduler = Scheduler::new();
    let flow = scheduler.register();
    let mut connection = Connection::new();
    for xid in 1..=20 {
        assert!(flow.enqueue(connection.command(xid, read_of(1 << 20)).await).is_ok());
    }

    for xid in 1..=20 {
        assert_eq!(next_xid(&scheduler).await, xid);
    }
}

#[tokio::test]
async fn backlogged_flows_get_equal_cost_shares() {
    let scheduler = Scheduler::new();
    let (light, heavy) = (scheduler.register(), scheduler.register());
    let (mut light_connection, mut heavy_connection) = (Connection::new(), Connection::new());
    for i in 0..500 {
        assert!(light.enqueue(light_connection.command(1000 + i, getattr()).await).is_ok());
        assert!(heavy.enqueue(heavy_connection.command(2000 + i, read_of(1 << 20)).await).is_ok());
    }

    let (mut light_cost, mut heavy_cost) = (0, 0);
    for _ in 0..200 {
        let command = scheduler.next().await.unwrap();
        if command.args.header.xid < 2000 {
            light_cost += cost(&command.args.proc);
        } else {
            heavy_cost += cost(&command.args.proc);
        }
    }
    // The heavy flow spends its quanta in steps of one READ, 17 units.
    let bound = QUANTUM + cost(&read_of(1 << 20));
    assert!(light_cost.abs_diff(heavy_cost) <= bound, "{light_cost} against {heavy_cost}");
}

#[tokio::test]
async fn a_light_flow_waits_for_at_most_one_quantum() {
    let scheduler = Scheduler::new();
    let (heavy, light) = (scheduler.register(), scheduler.register());
    let (mut heavy_connection, mut light_connection) = (Connection::new(), Connection::new());
    for i in 0..100 {
        assert!(heavy.enqueue(heavy_connection.command(1000 + i, getattr()).await).is_ok());
    }
    // The heavy flow is in the middle of its turn when the light one arrives.
    next_xid(&scheduler).await;
    assert!(light.enqueue(light_connection.command(1, getattr()).await).is_ok());

    let mut before = 0;
    while next_xid(&scheduler).await != 1 {
        before += 1;
    }
    assert!(before < QUANTUM, "the light flow waited for {before} procedures");
}

#[tokio::test]
async fn an_idle_flow_banks_no_credit() {
    let scheduler = Scheduler::new();
    let (first, second) = (scheduler.register(), scheduler.register());
    let (mut first_connection, mut second_connection) = (Connection::new(), Connection::new());
    // The first flow is served alone, then runs empty.
    for i in 0..40 {
        assert!(first.enqueue(first_connection.command(1000 + i, getattr()).await).is_ok());
    }
    for _ in 0..40 {
        next_xid(&scheduler).await;
    }
    for i in 0..20 {
        assert!(first.enqueue(first_connection.command(3000 + i, getattr()).await).is_ok());
        assert!(second.enqueue(second_connection.command(2000 + i, getattr()).await).is_ok());
    }

    let mut before = 0;
    while !(2000..3000).contains(&next_xid(&scheduler).await) {
        before += 1;
    }
    assert!(before <= QUANTUM, "the second flow waited for {before} procedures");
}

#[tokio::test]
async fn next_waits_for_a_procedure() {
    let scheduler = Scheduler::new();
    let flow = scheduler.register();
    let command = Connection::new().command(7, getattr()).await;

    let producer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(flow.enqueue(command).is_ok());
    });
    assert_eq!(next_xid(&scheduler).await, 7);
    producer.await.unwrap();
}

#[tokio::test]
async fn closing_hands_out_the_queued_procedures_then_stops() {
    let scheduler = Scheduler::new();
    let flow = scheduler.register();
    let mut connection = Connection::new();
    for xid in 1..=3 {
        assert!(flow.enqueue(connection.command(xid, getattr()).await).is_ok());
    }

    scheduler.close();
    assert!(flow.enqueue(connection.command(4, getattr()).await).is_err());
    for xid in 1..=3 {
        assert_eq!(next_xid(&scheduler).await, xid);
    }
    assert!(scheduler.next().await.is_none());
}

#[tokio::test]
async fn a_dropped_flow_is_freed_once_drained() {
    let scheduler = Scheduler::new();
    let flow = scheduler.register();
    let mut connection = Connection::new();
    for xid in 1..=2 {
        assert!(flow.enqueue(connection.command(xid, getattr()).await).is_ok());
    }

    drop(flow);
    assert_eq!(scheduler.flows(), 1, "a flow with queued procedures stays registered");
    assert_eq!(next_xid(&scheduler).await, 1);
    assert_eq!(scheduler.flows(), 1);
    assert_eq!(next_xid(&scheduler).await, 2);
    assert_eq!(scheduler.flows(), 0);

    drop(scheduler.register());
    assert_eq!(scheduler.flows(), 0, "an idle flow is freed right away");
}
