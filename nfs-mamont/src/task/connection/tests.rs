//! Tests of the in-flight limit over a loopback socket.
//!
//! The global tasks are replaced by channels the test reads from, so no `Vfs`, `Mount`
//! or `Nlm` implementation is needed.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_channel::Receiver;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use crate::allocator::{Allocator, Buffer};
use crate::connection_id::ConnectionId;
use crate::consts::nfsv3::{NfsProc, NFS3_FHSIZE, NFS_PROGRAM, NFS_VERSION};
use crate::consts::rpc::{HEADER_MASK, RPC_VERSION};
use crate::task::connection::limit::Budget;
use crate::task::connection::read::{CommandSenders, ReadTask};
use crate::task::connection::spawn_tasks;
use crate::task::connection::write::WriteTask;
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::nlm_task::NlmCommand;
use crate::task::global::vfs::VfsCommand;
use crate::task::{ProcReply, ProcResult};
use crate::vfs::NfsRes;

const LIMIT: usize = 4;
const WRITE_TIMEOUT: Duration = Duration::from_millis(100);

/// GETATTR carries no data, so nothing is ever allocated.
struct NoBuffer;

impl Buffer for NoBuffer {
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
        std::iter::empty()
    }

    fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
        std::iter::empty()
    }

    fn len(&self) -> usize {
        0
    }

    fn is_empty(&self) -> bool {
        true
    }

    fn empty() -> Self {
        NoBuffer
    }
}

struct NoAllocator;

impl Allocator for NoAllocator {
    type Buffer = NoBuffer;

    async fn allocate(&self, _size: NonZeroUsize) -> Option<NoBuffer> {
        None
    }

    fn capacity(&self) -> NonZeroUsize {
        NonZeroUsize::MIN
    }
}

/// Builds one record-marked GETATTR call with `AUTH_NONE` credentials and verifier.
fn getattr_frame(xid: u32) -> Vec<u8> {
    let call = [xid, 0, RPC_VERSION, NFS_PROGRAM, NFS_VERSION, NfsProc::GetAttr as u32];
    let auth = [0; 4]; // credentials and verifier: flavor and empty body each
    let mut body: Vec<u8> = call.into_iter().chain(auth).flat_map(u32::to_be_bytes).collect();
    body.extend_from_slice(&u32::try_from(NFS3_FHSIZE).unwrap().to_be_bytes());
    body.extend_from_slice(&[1; NFS3_FHSIZE]);
    body.resize(body.len().div_ceil(4) * 4, 0); // XDR padding of the handle

    let mut frame = u32::try_from(HEADER_MASK | body.len()).unwrap().to_be_bytes().to_vec();
    frame.append(&mut body);
    frame
}

struct Pipeline {
    client: TcpStream,
    pool_rx: Receiver<VfsCommand<NoBuffer>>,
    // Kept open so that the read task could still queue MOUNT and NLM commands.
    _mount_rx: Receiver<MountCommand<NoBuffer>>,
    _nlm_rx: Receiver<NlmCommand<NoBuffer>>,
}

/// Fills the socket until it stays unwritable, as if the client had stopped reading.
async fn fill_send_buffer(socket: &OwnedWriteHalf) {
    while timeout(Duration::from_millis(100), socket.writable()).await.is_ok() {
        let _ = socket.try_write(&[0; 64 * 1024]);
    }
}

/// Starts a connection and sends it twice as many GETATTR calls as it may have in flight.
async fn flooded_pipeline(client_reads: bool, write_timeout: Option<Duration>) -> Pipeline {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (client, accepted) = tokio::join!(TcpStream::connect(addr), listener.accept());
    let (socket, _) = accepted.unwrap();
    let (readhalf, writehalf) = socket.into_split();

    let (mount_tx, _mount_rx) = async_channel::unbounded();
    let (nlm_tx, _nlm_rx) = async_channel::unbounded();
    let (pool_tx, pool_rx) = async_channel::unbounded();
    let (result_tx, result_rx) = async_channel::unbounded();
    let budget = Budget::new(NonZeroUsize::new(LIMIT).unwrap(), result_tx);
    let senders = CommandSenders::new(mount_tx, nlm_tx, pool_tx);

    if !client_reads {
        fill_send_buffer(&writehalf).await;
    }
    let client_id = ConnectionId::new(addr);
    spawn_tasks(
        ReadTask::new(readhalf, client_id, Arc::new(NoAllocator), senders, budget),
        WriteTask::new(writehalf, result_rx, client_id, write_timeout),
    );

    let mut client = client.unwrap();
    let frames: Vec<u8> = (1..=2 * LIMIT as u32).flat_map(getattr_frame).collect();
    client.write_all(&frames).await.unwrap();

    Pipeline { client, pool_rx, _mount_rx, _nlm_rx }
}

/// Answers the `LIMIT` oldest queued commands; returns their xids.
async fn answer(pool_rx: &Receiver<VfsCommand<NoBuffer>>) -> Vec<u32> {
    let mut xids = Vec::with_capacity(LIMIT);
    for _ in 0..LIMIT {
        let VfsCommand { args, result_tx } = pool_rx.recv().await.unwrap();
        let xid = args.header.xid;
        let reply = ProcReply { xid, proc_result: Ok(ProcResult::Nfs3(Box::new(NfsRes::Null))) };
        result_tx.send(reply).await.unwrap();
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
async fn read_task_stops_reading_at_the_limit() {
    let pipeline = flooded_pipeline(true, None).await;

    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(pipeline.pool_rx.len(), LIMIT, "requests were read past the limit");

    // Written replies free their slots, and the rest of the requests get read.
    assert_eq!(answer(&pipeline.pool_rx).await, [1, 2, 3, 4]);
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;
    assert_eq!(answer(&pipeline.pool_rx).await, [5, 6, 7, 8]);

    drop(pipeline.client);
    wait_until(|| pipeline.pool_rx.is_closed()).await;
}

#[tokio::test]
async fn a_reply_that_cannot_be_written_keeps_its_slot() {
    let pipeline = flooded_pipeline(false, None).await;
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;

    assert_eq!(answer(&pipeline.pool_rx).await, [1, 2, 3, 4]);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(pipeline.pool_rx.len(), 0, "a slot was freed before its reply was written");
}

#[tokio::test]
async fn requests_dropped_without_a_reply_free_their_slots() {
    let pipeline = flooded_pipeline(true, None).await;
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;

    // What a panicking backend does with the command it executes.
    for _ in 0..LIMIT {
        drop(pipeline.pool_rx.recv().await.unwrap());
    }
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;
    assert_eq!(answer(&pipeline.pool_rx).await, [5, 6, 7, 8]);
}

#[tokio::test]
async fn a_client_that_does_not_read_is_disconnected() {
    let pipeline = flooded_pipeline(false, Some(WRITE_TIMEOUT)).await;
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;

    let stuck = Instant::now();
    answer(&pipeline.pool_rx).await;

    // The write task gives up and stops the read task, which closes the command channel.
    wait_until(|| pipeline.pool_rx.is_closed()).await;
    assert!(stuck.elapsed() >= WRITE_TIMEOUT, "closed after {:?}", stuck.elapsed());
}
