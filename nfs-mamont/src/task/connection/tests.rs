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
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;

use crate::allocator::{Allocator, Buffer};
use crate::connection_id::ConnectionId;
use crate::consts::nfsv3::{NfsProc, NFS3_FHSIZE, NFS_PROGRAM, NFS_VERSION};
use crate::consts::rpc::{HEADER_MASK, RPC_VERSION};
use crate::limits::connection::ConnectionLimits;
use crate::limits::probation::{Probation, Ticket};
use crate::limits::{self, Limits};
use crate::task::connection::read::{CommandSenders, ReadTask};
use crate::task::connection::spawn_tasks;
use crate::task::connection::write::WriteTask;
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::nlm_task::NlmCommand;
use crate::task::global::vfs::drr::{self, Slot};
use crate::task::global::vfs::{VfsCommand, VfsSender};
use crate::task::{ProcReply, ProcResult};
use crate::vfs::NfsRes;

const LIMIT: usize = 4;
const WRITE_TIMEOUT: Duration = Duration::from_millis(100);
const IDLE_TIMEOUT: Duration = Duration::from_millis(100);
/// How long the tests wait to see that something does not happen.
const WAIT: Duration = Duration::from_millis(50);
/// Bytes every READ of the tests asks for.
const READ_COUNT: u32 = 1000;
/// Allocator memory a test buffer holds per started unit, like a pool of whole buffers.
const UNIT: usize = 2048;

struct TestBuffer(Vec<u8>);

impl Buffer for TestBuffer {
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
        std::iter::once(&self.0[..])
    }

    fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
        std::iter::once(&mut self.0[..])
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn footprint(&self) -> usize {
        self.0.len().div_ceil(UNIT) * UNIT
    }

    fn empty() -> Self {
        TestBuffer(Vec::new())
    }
}

struct TestAllocator;

impl Allocator for TestAllocator {
    type Buffer = TestBuffer;

    async fn allocate(&self, size: NonZeroUsize) -> Option<TestBuffer> {
        Some(TestBuffer(vec![0; size.get()]))
    }

    fn capacity(&self) -> NonZeroUsize {
        NonZeroUsize::new(1 << 20).unwrap()
    }
}

/// Builds one record-marked call of `proc` on a file handle, followed by `args`, with
/// `AUTH_NONE` credentials and verifier.
fn call_frame(xid: u32, proc: NfsProc, args: &[u32]) -> Vec<u8> {
    let call = [xid, 0, RPC_VERSION, NFS_PROGRAM, NFS_VERSION, proc as u32];
    let auth = [0; 4]; // credentials and verifier: flavor and empty body each
    let mut body: Vec<u8> = call.into_iter().chain(auth).flat_map(u32::to_be_bytes).collect();
    body.extend_from_slice(&u32::try_from(NFS3_FHSIZE).unwrap().to_be_bytes());
    body.extend_from_slice(&[1; NFS3_FHSIZE]);
    body.resize(body.len().div_ceil(4) * 4, 0); // XDR padding of the handle
    body.extend(args.iter().flat_map(|word| word.to_be_bytes()));

    let mut frame = u32::try_from(HEADER_MASK | body.len()).unwrap().to_be_bytes().to_vec();
    frame.append(&mut body);
    frame
}

fn getattr_frame(xid: u32) -> Vec<u8> {
    call_frame(xid, NfsProc::GetAttr, &[])
}

/// READ of `READ_COUNT` bytes at offset 0.
fn read_frame(xid: u32) -> Vec<u8> {
    call_frame(xid, NfsProc::Read, &[0, 0, READ_COUNT])
}

struct Connection {
    // Dropped to hang up.
    _client: TcpStream,
    // Kept open so that the read task could still queue MOUNT and NLM commands.
    _mount_rx: Receiver<MountCommand<TestBuffer>>,
    _nlm_rx: Receiver<NlmCommand<TestBuffer>>,
}

struct Pipeline {
    connection: Connection,
    pool_rx: Pool,
}

/// The VFS pool of a test: the queues of its connections, read by the test instead of
/// the workers, behind `max_pending` slots.
struct Pool {
    sender: drr::Sender<VfsCommand<TestBuffer>>,
    receiver: drr::Receiver<VfsCommand<TestBuffer>>,
}

impl Pool {
    /// Opens the queue of a new connection.
    fn sender(&self) -> VfsSender<TestBuffer> {
        self.sender.flow()
    }

    /// Returns the number of commands queued over all connections.
    fn len(&self) -> usize {
        self.receiver.queued()
    }

    /// Returns whether every connection is gone; commands it queued may still wait for
    /// the workers.
    fn is_closed(&self) -> bool {
        self.receiver.senders() == 0
    }

    /// Takes the next command, in the order the workers would.
    async fn recv(&self) -> (VfsCommand<TestBuffer>, Slot) {
        self.receiver.recv().await.unwrap()
    }
}

fn pool(max_pending: Option<usize>) -> Pool {
    let (sender, receiver) = drr::channel(max_pending.map(|max| NonZeroUsize::new(max).unwrap()));
    Pool { sender, receiver }
}

/// Fills the socket until it stays unwritable, as if the client had stopped reading.
async fn fill_send_buffer(socket: &OwnedWriteHalf) {
    while timeout(Duration::from_millis(100), socket.writable()).await.is_ok() {
        let _ = socket.try_write(&[0; 64 * 1024]);
    }
}

/// How a test connection is set up, see [`flooded_connection`].
struct Setup {
    client_reads: bool,
    first_xid: u32,
    /// Data limit of the connection, see [`Limits::max_data_per_connection`].
    max_data: Option<usize>,
    write_timeout: Option<Duration>,
    idle_timeout: Option<Duration>,
    /// Place of the connection on probation, see [`Probation`].
    probation: Option<Ticket>,
    /// Slot of the connection in a connection limit.
    permit: Option<OwnedSemaphorePermit>,
    /// Builds the calls the client sends.
    call: fn(u32) -> Vec<u8>,
    /// How many calls the client sends.
    calls: u32,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            client_reads: true,
            first_xid: 1,
            max_data: None,
            write_timeout: None,
            idle_timeout: None,
            probation: None,
            permit: None,
            call: getattr_frame,
            calls: 2 * LIMIT as u32,
        }
    }
}

/// Starts a connection and sends it twice as many GETATTR calls as it may have in flight.
async fn flooded_pipeline(
    client_reads: bool,
    max_pending: Option<usize>,
    write_timeout: Option<Duration>,
) -> Pipeline {
    let pool_rx = pool(max_pending);
    let setup = Setup { client_reads, write_timeout, ..Setup::default() };
    Pipeline { connection: flooded_connection(pool_rx.sender(), setup).await, pool_rx }
}

/// Starts a connection queueing into `pool` and sends it `setup.calls` calls, with xids
/// from `setup.first_xid` on.
async fn flooded_connection(pool: VfsSender<TestBuffer>, setup: Setup) -> Connection {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (client, accepted) = tokio::join!(TcpStream::connect(addr), listener.accept());
    let (socket, _) = accepted.unwrap();
    let (readhalf, writehalf) = socket.into_split();

    let (mount_tx, _mount_rx) = async_channel::unbounded();
    let (nlm_tx, _nlm_rx) = async_channel::unbounded();
    let (result_tx, result_rx) = async_channel::unbounded();
    let limits = Limits {
        max_in_flight: NonZeroUsize::new(LIMIT).unwrap(),
        max_data_per_connection: setup.max_data.map(|max| NonZeroUsize::new(max).unwrap()),
        idle_timeout: setup.idle_timeout,
        ..limits::defaults(NonZeroUsize::MIN)
    };
    let connection_limits = ConnectionLimits::new(&limits, result_tx, setup.probation);
    let senders = CommandSenders::new(mount_tx, nlm_tx, pool);

    if !setup.client_reads {
        fill_send_buffer(&writehalf).await;
    }
    let client_id = ConnectionId::new(addr);
    let allocator = Arc::new(TestAllocator);
    spawn_tasks(
        ReadTask::new(readhalf, client_id, allocator, senders, connection_limits),
        WriteTask::new(writehalf, result_rx, client_id, setup.write_timeout),
        setup.permit,
    );

    let mut client = client.unwrap();
    let xids = setup.first_xid..setup.first_xid + setup.calls;
    client.write_all(&xids.flat_map(setup.call).collect::<Vec<_>>()).await.unwrap();

    Connection { _client: client, _mount_rx, _nlm_rx }
}

/// Answers the oldest queued command; returns its xid.
async fn answer_one(pool_rx: &Pool) -> u32 {
    // The slot of `max_pending` is held until the reply is queued, as the workers do.
    let (VfsCommand { args, result_tx }, _slot) = pool_rx.recv().await;
    let xid = args.header.xid;
    let reply = ProcReply { xid, proc_result: Ok(ProcResult::Nfs3(Box::new(NfsRes::Null))) };
    result_tx.send(reply).await.unwrap();
    xid
}

/// Answers the `LIMIT` oldest queued commands; returns their xids.
async fn answer(pool_rx: &Pool) -> Vec<u32> {
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
        drop(pipeline.pool_rx.recv().await);
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
    let pool_rx = pool(Some(1));
    let _first = flooded_connection(pool_rx.sender(), Setup::default()).await;
    let second = Setup { first_xid: 101, ..Setup::default() };
    let _second = flooded_connection(pool_rx.sender(), second).await;

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

    // The write task gives up and stops the read task, which drops its queue.
    wait_until(|| pipeline.pool_rx.is_closed()).await;
    assert!(stuck.elapsed() >= WRITE_TIMEOUT, "closed after {:?}", stuck.elapsed());
}

#[tokio::test]
async fn read_task_stops_reading_at_the_data_limit() {
    let pool_rx = pool(None);
    let setup = Setup { max_data: Some(3000), call: read_frame, ..Setup::default() };
    let _connection = flooded_connection(pool_rx.sender(), setup).await;

    // Every READ holds a whole unit of 2048 bytes, so two of them reach the limit of
    // 3000, before the four slots run out.
    wait_until(|| pool_rx.len() == 2).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(pool_rx.len(), 2, "requests were read past the data limit");

    // A written reply gives its bytes back, and the next READ is read.
    assert_eq!(answer_one(&pool_rx).await, 1);
    wait_until(|| pool_rx.len() == 2).await;
}

#[tokio::test]
async fn requests_larger_than_the_data_limit_are_read_one_at_a_time() {
    let pool_rx = pool(None);
    let setup = Setup { max_data: Some(1000), call: read_frame, ..Setup::default() };
    let _connection = flooded_connection(pool_rx.sender(), setup).await;

    for xid in 1..=3 {
        wait_until(|| pool_rx.len() == 1).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(pool_rx.len(), 1, "a READ larger than the limit was read with another");
        assert_eq!(answer_one(&pool_rx).await, xid);
    }
}

#[tokio::test]
async fn a_connection_that_sends_no_call_is_pushed_out() {
    let probation = Probation::new(NonZeroUsize::new(1).unwrap());
    let pool_rx = pool(None);
    let setup =
        Setup { call: |_| Vec::new(), probation: Some(probation.admit()), ..Setup::default() };
    let _silent = flooded_connection(pool_rx.sender(), setup).await;
    tokio::time::sleep(WAIT).await;
    assert!(!pool_rx.is_closed(), "the connection was closed before it was pushed out");

    let _newer = probation.admit();
    wait_until(|| pool_rx.is_closed()).await;
}

#[tokio::test]
async fn a_connection_that_sent_a_call_is_not_pushed_out() {
    let probation = Probation::new(NonZeroUsize::new(1).unwrap());
    let pool_rx = pool(None);
    let setup = Setup { probation: Some(probation.admit()), ..Setup::default() };
    let _connection = flooded_connection(pool_rx.sender(), setup).await;
    wait_until(|| pool_rx.len() == LIMIT).await;

    // Answered, so that the read task gets back to waiting for the socket, where a
    // connection on probation would be pushed out.
    let _newer = probation.admit();
    assert_eq!(answer(&pool_rx).await, [1, 2, 3, 4]);
    assert_eq!(answer(&pool_rx).await, [5, 6, 7, 8]);
    tokio::time::sleep(WAIT).await;
    assert!(!pool_rx.is_closed(), "a connection that sent calls was pushed out");
}

#[tokio::test]
async fn an_idle_connection_is_closed() {
    let pool_rx = pool(None);
    // Fewer calls than slots, so that the read task waits for the socket all along.
    let setup = Setup { idle_timeout: Some(IDLE_TIMEOUT), calls: 2, ..Setup::default() };
    let _connection = flooded_connection(pool_rx.sender(), setup).await;

    // Requests in flight keep the connection open past the timeout.
    wait_until(|| pool_rx.len() == 2).await;
    tokio::time::sleep(3 * IDLE_TIMEOUT).await;
    assert!(!pool_rx.is_closed(), "a connection with requests in flight was closed");

    // Once the last reply is written, the timeout starts.
    assert_eq!(answer_one(&pool_rx).await, 1);
    assert_eq!(answer_one(&pool_rx).await, 2);
    let idle = Instant::now();
    wait_until(|| pool_rx.is_closed()).await;
    assert!(idle.elapsed() >= IDLE_TIMEOUT, "closed after {:?}", idle.elapsed());
}

#[tokio::test]
async fn the_connection_slot_is_given_back_when_the_connection_closes() {
    let slots = Arc::new(Semaphore::new(1));
    let pool_rx = pool(None);
    let permit = Some(Arc::clone(&slots).try_acquire_owned().unwrap());
    let connection =
        flooded_connection(pool_rx.sender(), Setup { permit, ..Setup::default() }).await;
    assert_eq!(answer(&pool_rx).await, [1, 2, 3, 4]);
    assert_eq!(answer(&pool_rx).await, [5, 6, 7, 8]);
    assert_eq!(slots.available_permits(), 0, "the slot is held while the connection is open");

    drop(connection);
    wait_until(|| slots.available_permits() == 1).await;
}
