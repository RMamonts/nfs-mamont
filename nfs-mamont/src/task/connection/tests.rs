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
use tokio::sync::Semaphore;
use tokio::time::timeout;

use crate::allocator::{Allocator, Buffer};
use crate::connection_id::ConnectionId;
use crate::consts::nfsv3::{NfsProc, NFS3_FHSIZE, NFS_PROGRAM, NFS_VERSION};
use crate::consts::rpc::{HEADER_MASK, RPC_VERSION};
use crate::metric::{self, CountedReceiver, Timer};
use crate::task::connection::limit::Budget;
use crate::task::connection::read::{CommandSenders, ReadTask};
use crate::task::connection::spawn_tasks;
use crate::task::connection::write::WriteTask;
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::nlm_task::NlmCommand;
use crate::task::global::vfs::{VfsCommand, VfsSender};
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

struct Connection {
    // Dropped to hang up.
    _client: TcpStream,
    // Kept open so that the read task could still queue MOUNT and NLM commands.
    _mount_rx: Receiver<MountCommand<NoBuffer>>,
    _nlm_rx: Receiver<NlmCommand<NoBuffer>>,
}

struct Pipeline {
    connection: Connection,
    pool_rx: CountedReceiver<VfsCommand<NoBuffer>>,
}

/// The VFS pool of a test: a queue the test reads from, behind `max_pending` slots.
fn pool(
    max_pending: Option<usize>,
    metrics: Arc<metric::NfsMetrics>,
) -> (VfsSender<NoBuffer>, CountedReceiver<VfsCommand<NoBuffer>>) {
    let (tx, rx) = metric::unbounded(metrics, metric::Queue::Vfs);
    let pending = max_pending.map(|permits| Arc::new(Semaphore::new(permits)));
    (VfsSender::new(tx, pending), rx)
}

/// Fills the socket until it stays unwritable, as if the client had stopped reading.
async fn fill_send_buffer(socket: &OwnedWriteHalf) {
    while timeout(Duration::from_millis(100), socket.writable()).await.is_ok() {
        let _ = socket.try_write(&[0; 64 * 1024]);
    }
}

/// Starts a connection and sends it twice as many GETATTR calls as it may have in flight.
async fn flooded_pipeline(
    client_reads: bool,
    max_pending: Option<usize>,
    write_timeout: Option<Duration>,
) -> Pipeline {
    let metrics = Arc::new(metric::NfsMetrics::new());
    let (pool, pool_rx) = pool(max_pending, metrics.clone());
    Pipeline {
        connection: flooded_connection(pool, client_reads, 1, write_timeout, metrics).await,
        pool_rx,
    }
}

/// Starts a connection queueing into `pool` and sends it twice as many GETATTR calls as
/// it may have in flight, with xids from `first_xid` on.
async fn flooded_connection(
    pool: VfsSender<NoBuffer>,
    client_reads: bool,
    first_xid: u32,
    write_timeout: Option<Duration>,
    metrics: Arc<metric::NfsMetrics>,
) -> Connection {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (client, accepted) = tokio::join!(TcpStream::connect(addr), listener.accept());
    let (socket, _) = accepted.unwrap();
    let (readhalf, writehalf) = socket.into_split();

    let (mount_tx, _mount_rx) = async_channel::unbounded();
    let (nlm_tx, _nlm_rx) = async_channel::unbounded();
    let (result_tx, result_rx) = metric::unbounded(metrics.clone(), metric::Queue::Reply);
    let budget = Budget::new(NonZeroUsize::new(LIMIT).unwrap(), result_tx);
    let senders = CommandSenders::new(mount_tx, nlm_tx, pool);

    if !client_reads {
        fill_send_buffer(&writehalf).await;
    }
    let client_id = ConnectionId::new(addr);
    spawn_tasks(
        ReadTask::new(readhalf, client_id, Arc::new(NoAllocator), senders, metrics.clone(), budget),
        WriteTask::new(writehalf, result_rx, metrics.clone(), client_id, write_timeout),
    );

    let mut client = client.unwrap();
    let xids = first_xid..first_xid + 2 * LIMIT as u32;
    client.write_all(&xids.flat_map(getattr_frame).collect::<Vec<_>>()).await.unwrap();

    Connection { _client: client, _mount_rx, _nlm_rx }
}

/// Answers the oldest queued command; returns its xid.
async fn answer_one(pool_rx: &CountedReceiver<VfsCommand<NoBuffer>>) -> u32 {
    let VfsCommand { args, result_tx, .. } = pool_rx.recv().await.unwrap();
    let xid = args.header.xid;
    let reply = ProcReply {
        xid,
        proc_result: Ok(ProcResult::Nfs3(Box::new(NfsRes::Null))),
        timer: Timer::NONE,
    };
    result_tx.send(reply).await.unwrap();
    xid
}

/// Answers the `LIMIT` oldest queued commands; returns their xids.
async fn answer(pool_rx: &CountedReceiver<VfsCommand<NoBuffer>>) -> Vec<u32> {
    let mut xids = Vec::with_capacity(LIMIT);
    for _ in 0..LIMIT {
        xids.push(answer_one(pool_rx).await);
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
    let pipeline = flooded_pipeline(true, None, None).await;

    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(pipeline.pool_rx.len(), LIMIT, "requests were read past the limit");

    // Written replies free their slots, and the rest of the requests get read.
    assert_eq!(answer(&pipeline.pool_rx).await, [1, 2, 3, 4]);
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;
    assert_eq!(answer(&pipeline.pool_rx).await, [5, 6, 7, 8]);

    drop(pipeline.connection);
    wait_until(|| pipeline.pool_rx.is_closed()).await;
}

#[tokio::test]
async fn a_reply_that_cannot_be_written_keeps_its_slot() {
    let pipeline = flooded_pipeline(false, None, None).await;
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;

    assert_eq!(answer(&pipeline.pool_rx).await, [1, 2, 3, 4]);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(pipeline.pool_rx.len(), 0, "a slot was freed before its reply was written");
}

#[tokio::test]
async fn requests_dropped_without_a_reply_free_their_slots() {
    let pipeline = flooded_pipeline(true, None, None).await;
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;

    // What a panicking backend does with the command it executes.
    for _ in 0..LIMIT {
        drop(pipeline.pool_rx.recv().await.unwrap());
    }
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;
    assert_eq!(answer(&pipeline.pool_rx).await, [5, 6, 7, 8]);
}

#[tokio::test]
async fn the_pool_holds_at_most_max_pending_requests() {
    let pipeline = flooded_pipeline(true, Some(2), None).await;

    wait_until(|| pipeline.pool_rx.len() == 2).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(pipeline.pool_rx.len(), 2, "requests were queued past the limit");

    // Each answered request frees its slot, and the next one is queued.
    assert_eq!(answer(&pipeline.pool_rx).await, [1, 2, 3, 4]);
}

#[tokio::test]
async fn pool_slots_go_round_the_waiting_connections() {
    let metrics = Arc::new(metric::NfsMetrics::new());
    let (pool, pool_rx) = pool(Some(1), metrics.clone());
    let _first = flooded_connection(pool.clone(), true, 1, None, metrics.clone()).await;
    let _second = flooded_connection(pool, true, 101, None, metrics.clone()).await;

    // The first connection queued its second request before the second one connected,
    // so the turns start with the second command.
    let mut owners = Vec::new();
    for _ in 0..2 * LIMIT {
        // Let both read tasks get back to waiting for the slot.
        tokio::time::sleep(Duration::from_millis(20)).await;
        owners.push(answer_one(&pool_rx).await > 100);
    }
    let turns = &owners[1..];
    assert!(turns.windows(2).all(|pair| pair[0] != pair[1]), "slots went {owners:?}");
}

#[tokio::test]
async fn a_client_that_does_not_read_is_disconnected() {
    let pipeline = flooded_pipeline(false, None, Some(WRITE_TIMEOUT)).await;
    wait_until(|| pipeline.pool_rx.len() == LIMIT).await;

    let stuck = Instant::now();
    answer(&pipeline.pool_rx).await;

    // The write task gives up and stops the read task, which closes the command channel.
    wait_until(|| pipeline.pool_rx.is_closed()).await;
    assert!(stuck.elapsed() >= WRITE_TIMEOUT, "closed after {:?}", stuck.elapsed());
}
