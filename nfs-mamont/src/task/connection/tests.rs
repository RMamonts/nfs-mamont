//! Tests of the connection pipeline over a loopback socket.
//!
//! The global tasks are replaced by channels the test reads from, so no `Vfs`, `Mount`
//! or `Nlm` implementation is needed.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_channel::Receiver;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

use crate::allocator::{Impl, Slice};
use crate::consts::nfsv3::{NfsProc, NFS3_FHSIZE, NFS_PROGRAM, NFS_VERSION};
use crate::consts::rpc::RPC_VERSION;
use crate::task::connection::limit::{Budget, Limiter};
use crate::task::connection::read::{CommandSenders, ReadTask};
use crate::task::connection::write::WriteTask;
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::nlm_task::NlmCommand;
use crate::task::global::vfs::VfsCommand;
use crate::task::{InFlightReply, ProcReply, ProcResult};
use crate::vfs::NfsRes;

/// In-flight budget of the test connection.
const LIMIT: usize = 4;

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

fn push_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_be_bytes());
}

/// Builds one record-marked GETATTR call with `AUTH_NONE` credentials and verifier.
fn getattr_frame(xid: u32) -> Vec<u8> {
    let mut body = Vec::new();
    push_u32(&mut body, xid);
    push_u32(&mut body, 0); // msg_type CALL
    push_u32(&mut body, RPC_VERSION);
    push_u32(&mut body, NFS_PROGRAM);
    push_u32(&mut body, NFS_VERSION);
    push_u32(&mut body, NfsProc::GetAttr as u32);
    push_u32(&mut body, 0); // cred: AUTH_NONE
    push_u32(&mut body, 0); // cred body length
    push_u32(&mut body, 0); // verf: AUTH_NONE
    push_u32(&mut body, 0); // verf body length
    push_u32(&mut body, u32::try_from(NFS3_FHSIZE).unwrap());
    body.extend_from_slice(&[0x01; NFS3_FHSIZE]);
    body.resize(body.len().div_ceil(4) * 4, 0); // XDR padding of the handle

    let mut frame = Vec::with_capacity(body.len() + 4);
    push_u32(&mut frame, 0x8000_0000 | u32::try_from(body.len()).unwrap());
    frame.extend_from_slice(&body);
    frame
}

/// A connection pipeline over a loopback socket with the global tasks replaced by
/// channels the test reads from.
struct Pipeline {
    client: TcpStream,
    pool_rx: Receiver<VfsCommand<Slice>>,
    limiter: Arc<Limiter>,
    // Kept open so that the read task could still queue MOUNT and NLM commands.
    _mount_rx: Receiver<MountCommand<Slice>>,
    _nlm_rx: Receiver<NlmCommand<Slice>>,
}

async fn pipeline(with_write_task: bool) -> Pipeline {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (client, accepted) = tokio::join!(TcpStream::connect(addr), listener.accept());
    let client = client.unwrap();
    let (socket, _) = accepted.unwrap();
    let (readhalf, writehalf) = socket.into_split();

    let (mount_tx, _mount_rx) = async_channel::unbounded();
    let (nlm_tx, _nlm_rx) = async_channel::unbounded();
    let (pool_tx, pool_rx) = async_channel::unbounded();
    let (result_tx, result_rx) = async_channel::unbounded();
    let (budget, limiter) = Budget::new(nonzero(LIMIT));
    let allocator = Arc::new(Impl::new(nonzero(4096), nonzero(1)));

    let senders = CommandSenders::new(mount_tx, nlm_tx, pool_tx, result_tx);
    ReadTask::new(readhalf, addr, allocator, senders, budget).spawn();
    if with_write_task {
        WriteTask::new(writehalf, result_rx, Arc::clone(&limiter)).spawn();
    }
    // Without a write task `result_rx` is dropped here, so no reply can be queued.

    Pipeline { client, pool_rx, limiter, _mount_rx, _nlm_rx }
}

/// Takes the `count` oldest queued commands and answers each with a NULL reply through
/// the channel it carries; returns their xids.
async fn answer(pool_rx: &Receiver<VfsCommand<Slice>>, count: usize) -> Vec<u32> {
    let mut xids = Vec::with_capacity(count);
    for _ in 0..count {
        let VfsCommand { args, result_tx, token } = pool_rx.recv().await.unwrap();
        let xid = args.header.xid;
        let reply = ProcReply { xid, proc_result: Ok(ProcResult::Nfs3(Box::new(NfsRes::Null))) };
        result_tx.send(InFlightReply { reply, token }).await.unwrap();
        xids.push(xid);
    }
    xids
}

/// Polls `condition` for up to five seconds.
async fn wait_until(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "condition not met within 5 s");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
async fn read_task_stops_reading_at_the_in_flight_limit() {
    let mut pipeline = pipeline(true).await;
    let frames: Vec<u8> = (1..=2 * LIMIT as u32).flat_map(getattr_frame).collect();
    pipeline.client.write_all(&frames).await.unwrap();

    // LIMIT requests are parsed and queued, then the read task parks on the budget
    // instead of reading the rest.
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(pipeline.pool_rx.len(), LIMIT, "requests were read past the limit");
    assert_eq!(pipeline.limiter.released(), 0);

    // Answering them makes the write task return their tokens, and the rest get read.
    assert_eq!(answer(&pipeline.pool_rx, LIMIT).await, [1, 2, 3, 4]);
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;
    assert_eq!(answer(&pipeline.pool_rx, LIMIT).await, [5, 6, 7, 8]);
    wait_until(|| pipeline.limiter.released() == 2 * LIMIT as u64).await;

    // Once the client hangs up the read task exits, the result channel drains and the
    // write task stops and closes the budget.
    drop(pipeline.client);
    wait_until(|| pipeline.limiter.is_closed()).await;
}

#[tokio::test]
async fn read_task_stops_when_the_budget_is_closed() {
    let mut pipeline = pipeline(false).await;
    let frames: Vec<u8> = (1..=2 * LIMIT as u32).flat_map(getattr_frame).collect();
    pipeline.client.write_all(&frames).await.unwrap();
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;

    // The read task is parked on the exhausted budget; closing it (what the write task
    // does when it stops) makes the read task exit, which closes the command channel.
    pipeline.limiter.close();
    wait_until(|| pipeline.pool_rx.is_closed()).await;
    assert_eq!(pipeline.pool_rx.len(), LIMIT, "queued commands stay readable");
}
