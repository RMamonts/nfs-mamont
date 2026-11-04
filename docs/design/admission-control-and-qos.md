# An upper bound for the server, QoS between clients, and the role of the allocator

Status: proposal for discussion, 2026-11-04.

Starting point: [PR #450](https://github.com/RMamonts/nfs-mamont/pull/450) (per-connection
in-flight limit) and the report `research/in-flight-limit/README.md` from the history of the
branch `vyar/in_flight_limit` (commit `a217319`; "the report" below). Related issues:
[#102](https://github.com/RMamonts/nfs-mamont/issues/102) Server limits,
[#103](https://github.com/RMamonts/nfs-mamont/issues/103) Allocation limits,
[#134](https://github.com/RMamonts/nfs-mamont/issues/134) Bounded channels,
[#219](https://github.com/RMamonts/nfs-mamont/issues/219) limit async_channel to vfs_pool,
[#327](https://github.com/RMamonts/nfs-mamont/issues/327) /
[#419](https://github.com/RMamonts/nfs-mamont/issues/419) client identification,
[#374](https://github.com/RMamonts/nfs-mamont/issues/374) metrics,
[#351](https://github.com/RMamonts/nfs-mamont/issues/351) multi backend.

## TL;DR

1. The limit of PR #450 stays: a slot per connection that is given back only after the reply
   is written to the socket is the only way to bound a *slow reader*. But its bound,
   `max_in_flight × connections`, grows with the number of connections, so global budgets
   are needed on top of it.
2. Three global budgets, each tied to one resource and one release point:
   - `max_connections`: connections (taken at `accept`);
   - `max_pending`: requests parsed by a read task but not yet finished by a worker
     (≈ `4 × vfs_concurrency`): this is the "reasonable upper bound of the VFS queue";
   - `data_budget`: bytes of allocator buffers in flight, below the pool capacity; plus
     `max_data_per_connection`.
3. The back-pressure point is the read task: it stops reading the socket, TCP closes the
   window, the client's slot table fills up. The allocator becomes the *floor*: with consistent
   budgets `allocate()` does not block; if it does, that is an anomaly (warn + metric), not the
   normal mode.
4. QoS: the shared FIFO VFS queue is replaced by a queue per flow (connection, later client)
   and a DRR (deficit round robin) dispatcher with a byte-based cost per request. The scheme
   is work-conserving: the share a flow does not use goes to the others; under contention the
   shares are equal. This gives the isolation of §4.7 of the report (p50 of the light client
   1.3 ms instead of 4.5) *without* lowering `max_in_flight`, which cost the heavy client −14 %.
5. Three phases: (1) global budgets on top of #450; (2) the DRR dispatcher; (3) write deadline,
   measured cost, flow key = client, READ buffer allocation at dispatch time.

## 1. What exists today

```text
socket ─▶ ReadTask ──VfsCommand──▶ [async_channel, unbounded, FIFO] ──▶ VfsTask × vfs_concurrency
           │  parse + allocate()                                            │ backend.<proc>()
           └── NULL / parse error ──▶ [result channel, unbounded] ◀─────────┘
                                               │
                                           WriteTask ─▶ socket
```

| Resource | `main` | PR #450 |
|---|---|---|
| VFS workers | `vfs_concurrency` | same |
| VFS queue (and MOUNT/NLM) | unbounded | ≤ `max_in_flight × connections` |
| Replies of one connection | unbounded: a client that does not read its replies grows the server to 1.6–3.0 GiB in 15 s (report §4.5) | ≤ `max_in_flight`, RSS 4.4 MiB |
| Allocator buffers | pool semaphore, FIFO, blocks the read task inside the parser | same: the limit counts requests, not bytes (§4.6) |
| Connections | unbounded | unbounded |

Why blocking in the allocator is a bad back-pressure (`PoolAllocator::allocate`,
`nfs-mamont-util/src/allocator/mod.rs:90`; called from `adapter_for_read` and
`adapter_for_write`, `nfs-mamont/src/parser/parser_struct.rs`):

- It happens inside `next_message()`, after the header is read and the connection slot is
  taken. The whole connection stalls, including its cheap metadata operations, which need no
  memory at all.
- `tokio::sync::Semaphore` is fair by the order of *requests*, not of *clients*: one
  `acquire_many` for 1 MiB at the head of the queue holds every small request behind it. A
  client that sends READ 1 MiB and never reads the replies takes the whole pool: a "victim"
  doing READ 4 KiB gets 0 ops/s both on `main` and on the branch with the default of 256 (§4.6).
- The allocator does not know which connection a buffer belongs to, so it cannot bound the
  share of one client. A limit in requests does not help: 256 × 1 MiB is more than a 64 MiB
  pool.
- The pool hands out whole buffers (`count_needed = ceil(size / buffer_size)`): a READ of
  4 KiB takes 1 MiB. Any accounting "in request bytes" that ignores the pool granularity
  underestimates its consumption.

## 2. Requirements and their reading

| | Requirement | What it means for the implementation |
|---|---|---|
| R1 | A reasonable upper bound for the server | The bound must not depend on the number of connections: memory, queue depth and the number of connections are bounded by configuration constants. |
| R2 | QoS for every client | A heavy or slow client does not increase the latency of the others and cannot take a shared resource (workers, memory, the buffer pool) entirely. |
| R3 | The fast client gets more, not an equal split | The division is not static but max-min: a resource is split equally only among those who *ask* for it; the unused share goes to the others. Two special cases: (a) the slow client is limited by its link or by the depth of its own queue and asks for little, the fast one takes the rest; (b) the requests of the slow client are more expensive (large READs, cold data): with an equal share of *worker time* the fast one gets more *requests*. A work-conserving scheduler with a request cost covers both. |
| R4 | The allocator is not a back-pressure mechanism but the last line of defence | The decision "read the next request or not" is made before allocation, fairly between connections and with bytes accounted for. The byte budget is below the pool, so `allocate()` does not wait in the normal mode. The pool semaphore stays: it protects against what the server cannot account for (a backend holding buffers, a wrong configuration). |

Why block the read task at all: NFSv3 has no RPC-level credits (they appear only with NFSv4.1
sessions), so the only signal to the client is TCP flow control. A Linux client keeps up to
65 536 RPCs per TCP connection, so the server has to be the one that limits. Stopping the read
of the socket is the right back-pressure; what is bad is when it happens *uncontrolled* and
*unfairly* (in the allocator), not when it happens at all.

## 3. Model: stages, resources, release points

| Stage | What is held | Who bounds it | When it is released |
|---|---|---|---|
| Parsing (read task) | parser buffer 2.5 KiB, connection slot | `max_connections`, `max_in_flight` | end of connection; reply written |
| Waiting for a worker | parsed arguments, READ/WRITE buffer | `max_pending`, `data_budget`, `max_data_per_connection` | worker finished; buffer returned |
| Execution | worker, buffer | `vfs_concurrency` | `dispatch` returned |
| Waiting to be written | reply (≈300 B, plus the buffer for READ), connection slot | `max_in_flight`, `max_data_per_connection`, `write_timeout` | `write_all` finished |

The key split: the *worker* budget (`max_pending`) is released when the worker is done, the
*connection* budget (`max_in_flight`, bytes) when the reply is written. So a slow reader holds
only its own slots and its own bytes and does not take the global queue budget.

## 4. Proposal

### 4.1 The token as the lease of a request's resources

The `Token` of PR #450 (`task/connection/limit.rs`) already travels with the request and frees
the slot on `Drop`. It is extended into a "lease": connection slot + bytes (local and global).
Release on `Drop` stays the only return point, so a request destroyed at any stage (a panicking
backend, a closed channel) gives back everything it held.

```rust
pub struct Token {
    limiter: Arc<Limiter>,   // slots and bytes of the connection, as in #450
    bytes: usize,            // how many bytes of data_budget are leased
    global: Arc<Limits>,     // global budgets
}
```

The `max_pending` budget is not part of the token: it has a different release point. It travels
in the command as a permit and is dropped at the end of `dispatch` (after `result_tx.send`).

### 4.2 Global budgets

| Budget | Unit | Taken | Released | Default |
|---|---|---|---|---|
| `max_connections` | connection | `handle_forever`, before `accept()` | both tasks of the connection finished | 1024 (below `RLIMIT_NOFILE`) |
| `max_pending` | request | read task, after `next_message()`, before queueing | end of `dispatch` (VFS), end of processing in MOUNT/NLM; NULL and parse errors take no permit | `max(4 × vfs_concurrency, max_in_flight)` |
| `data_budget` | byte, rounded up to `granularity` | inside the wrapper's `allocate()`, before the allocator is called | `Token::drop` (READ: after the reply is written; WRITE: can be earlier, see 4.3) | explicit; with the pool, 7/8 of its size |

`max_pending` is taken after parsing, not before reading the frame: otherwise every *idle*
connection would hold a global slot "in reserve". The connection slot (`max_in_flight`) can be
taken before reading, as in #450: an empty slot of an idle connection costs nothing.

Why `4 × vfs_concurrency`: the workers are busy while the queue holds `vfs_concurrency` ready
requests plus a margin for the pipeline; everything beyond that is pure queueing delay, not
throughput (doc comment of `consts/connection.rs` in #450: one client against 64 workers and a
1 ms backend gets 44k ops/s with a limit of 64 and 58k with 128 and 256). The lower bound
`max_in_flight` is needed so that one connection can keep the pool busy.

Waiting on a budget: every connection has at most one waiter (its read task), so FIFO by
`acquire` calls is round robin by *connections*. `tokio::sync::Semaphore` is enough (55–68 ns
against 13 ns of the counter of #450, < 0.5 % end to end by the report §3); or the same
counter-based `Budget`, extended with a list of waiters.

Policy at `accept`: wait for a permit before `accept()` (the kernel backlog absorbs the burst,
no extra `accept` + `close`) and log a rate-limited warning when the budget is exhausted. The
knfsd alternative (`svc_check_conn_limits`: close the oldest connection) is useful against dead
clients but needs idle accounting; leave it for later together with an idle timeout.

### 4.3 Byte budget and the allocator

The wrapper `BudgetedAllocator<A>` implements `Allocator` and is given to `RpcParser` instead
of the user's allocator (the parser is generic over `A`, nothing in it changes):

```rust
async fn allocate(&self, size: NonZeroUsize) -> Option<B> {
    let charge = round_up(size, self.inner.granularity());
    self.connection.acquire_bytes(charge).await;   // max_data_per_connection
    self.global.acquire_bytes(charge).await;       // data_budget, round robin over connections
    let started = Instant::now();
    let buffer = self.inner.allocate(size).await;  // normally does not wait
    if started.elapsed() > ALLOCATOR_WAIT_WARN { warn!(...); metrics.allocator_waits += 1; }
    self.pending_charge.set(charge);
    buffer
}
```

After `next_message()` the read task takes `pending_charge` and puts it into the token; if
parsing failed, the buffer was already destroyed by the parser and the charge is returned right
away. Since the read task is sequential, there is always exactly one "current" charge.

Granularity: add to `Allocator` a method with a default implementation
`fn granularity(&self) -> NonZeroUsize { 1 }` (does not break foreign implementations) and make
`PoolAllocator` return `buffer_size`. The budget then counts what the pool really spends, and a
READ of 4 KiB costs 1 MiB, as it does in reality. (That a READ of 4 KiB takes 1 MiB at all is a
separate topic about size classes in the pool.)

The per-connection limit `max_data_per_connection` is counted the same way as the slots in #450
(a local issued/returned counter, an atomic counter of the returned part in `Limiter`). Default
32 MiB: a client needs about `bandwidth × (RTT + server latency)`; for 10 GbE and 5 ms that is
6–8 MiB, 32 MiB covers 25 GbE.

Early return for WRITE: the buffer goes to the backend and returns to the pool when
`backend.write()` is done, while the reply is small. The worker can call
`token.release_bytes()` right after `execute` so that WRITE bytes are not kept in the budget
until the reply is written to a slow client.

### 4.4 Fair VFS dispatcher

Instead of one `async_channel`: a `Scheduler` with a queue per flow and a DRR pick:

```rust
struct Scheduler<B> { inner: Mutex<Inner<B>>, wake: Notify, quantum: u32, closed: AtomicBool }
struct Inner<B>     { flows: Slab<Flow<B>>, active: VecDeque<FlowId> }
struct Flow<B>      { queue: VecDeque<VfsCommand<B>>, deficit: i64, active: bool }
```

- `enqueue(flow, cmd)`: push to the flow's queue; if the flow was not active, append it to the
  tail of `active`; `wake.notify_one()`.
- `next()` (worker): the flow at the head of `active`; if its queue is empty, `deficit = 0` and
  remove it from `active`; if `deficit ≥ cost(head)`, `deficit -= cost` and return the request
  (the flow stays at the head, the next free worker continues with it); otherwise
  `deficit += quantum` and move the flow to the tail. All under one mutex, O(1) amortised. If
  `active` is empty, wait on `wake`.
- `cost(cmd) = 1 + ceil(bytes / 64 KiB)`: GETATTR = 1, READ 64 KiB = 2, READ 1 MiB = 17.
  `quantum = 32`: per round a flow with metadata gets up to 32 operations, a flow with READ
  1 MiB about two. Equal *cost* per round is exactly what R3(b) needs.
- Flows are registered when a connection is created (`FlowHandle` in `CommandSenders` instead
  of `pool_sender`) and removed when it closes; the commands of a closed flow are finished.
- `vfs_concurrency` workers run `while let Some(cmd) = scheduler.next().await { dispatch(cmd) }`.
  `next()` returns `None` when `VfsManager` is destroyed and the queues are empty, the same
  semantics as a closed channel today.

Properties: work-conserving (a single active flow gets all workers); under contention backlogged
flows get equal cost shares; the latency of a light flow is about one round over the active
flows, not the depth of the heavy flow's queue; an idle flow banks no credit (`deficit = 0`
when its queue empties). The workers wait for nothing but the backend: they block neither on a
connection channel (it stays unbounded, its depth bounded by tokens) nor on the budgets.

Flow key: the connection in the first step. A Linux client with `nconnect=16` gets 16 shares;
once `ClientId` (#327/#419) exists, the key should become the client, while the connection
slots stay per connection (the write task is per socket).

### 4.5 Acquisition order and absence of deadlocks

The read task acquires strictly in one order:
`connection → connection slot → [parsing: connection bytes → data_budget → allocate] → max_pending → enqueue`.

Releases are done by those who acquire nothing: the worker (`max_pending`), the write task
(slot and bytes through `Token::drop`), `Slice::drop` (the pool). There is no cycle: any
waiting read task waits only for the progress of workers and reply writes, and those depend on
no budget. The only infinite waiter is a client that does not read its replies; it is closed by
`write_timeout`.

Head-of-line within one connection (a large WRITE waits for bytes, a GETATTR behind it waits for
the WRITE) cannot be removed with sequential parsing of a TCP stream: a frame cannot be skipped
without reading its data. It is softened by the budget sizes and the fair hand-out; for READ it
is removed by allocating the buffer at dispatch time (§11).

### 4.6 Slow reader and the write deadline

Three bounds per connection: `max_in_flight` requests, `max_data_per_connection` bytes and
`write_timeout` on `form_reply` in the write task. On timeout: warn, leave the write loop,
`CloseOnDrop` closes the budget, the read task finishes, the tokens are destroyed, everything is
returned. knfsd does the same: `sk_sndtimeo = 30 s` on the accepted socket and closing the
connection on an incomplete send of a reply (`svc_tcp_sendto`: "... when sending N bytes -
shutting down socket"). Default 30 s, `0` disables it.

Without the deadline the sum of `max_data_per_connection` over slow readers can exhaust
`data_budget` (with 32 MiB and a budget of 896 MiB: 28 non-reading clients); the deadline makes
that state temporary.

### 4.7 What is left to the allocator

The pool semaphore is not touched: it is the floor. `allocate()` waits only when (1)
`data_budget` is larger than the free capacity of the pool: check at start-up and log a warning;
(2) the backend keeps buffers longer than the request lives (cache, write-behind), which the
server cannot account for; (3) a foreign `Allocator` did not implement `granularity()` and hands
out more than asked. All three are anomalies, visible through the `allocator_waits` counter and
a warning in the log.

## 5. Behaviour in scenarios

| Scenario | Today (`main` / #450) | Proposal |
|---|---|---|
| One client, deep queue | 143k ops/s when the limit is reached (§4.1) | the same: DRR with one flow is FIFO, the budgets are not reached |
| Heavy (depth 2048) + light (depth 1), backend 1 ms | light p50 35.8 ms / 4.5 ms with limit 256; 1.3 ms with limit 64 at −14 % for the heavy one (§4.7) | light ≈ 1–1.5 ms with limit 256: its request waits one DRR round; the heavy one loses nothing |
| Fast + slow (narrow link) client | FIFO: the slow one is limited only by its replies in the write queue | the slots of the slow one get stuck in the write queue, its VFS queue is almost empty; DRR gives the workers to the fast one; with equal demand 50/50 |
| Fast (GETATTR) + slow (READ 1 MiB, cold) | FIFO by requests: large READs take the workers | equal cost per round: 32 GETATTR against ~2 READ; the fast one gets more requests |
| 1000 connections, all active | queue of up to 256 000 requests, ~130 MiB, latency = depth / throughput | ≤ `max_pending` (256) materialised; the rest in the sockets; round robin over connections at `max_pending` |
| Client does not read replies (GETATTR) | 1.6 GiB in 15 s / 4.4 MiB (#450) | 4.4 MiB, the connection closed after 30 s |
| Client sends READ 1 MiB and does not read (§4.6) | victim READ 4 KiB: 0 ops/s with limit 256 | the hog holds ≤ 32 MiB and ≤ 256 slots; the victim ≈ 59k ops/s; after 30 s the hog is closed |
| Backend 20 ms, 1024 workers, one client (§4.3) | 12.5k ops/s with limit 256 (−74 %) | as in #450: the per-connection limit is still `max(256, 2 × vfs_concurrency)` = 2048, `max_pending` = 4096 |

## 6. Parameters and defaults

| Parameter | Default | Rationale |
|---|---|---|
| `max_in_flight` (#450) | `max(256, 2 × vfs_concurrency)` | one connection keeps the pool busy (report §4.3) |
| `max_connections` | 1024 | bound memory per connection and FDs; align with `RLIMIT_NOFILE` |
| `max_pending` | `max(4 × vfs_concurrency, max_in_flight)` | Little's law: more only adds latency |
| `data_budget` | set explicitly; with `PoolAllocator`, 7/8 of the pool | margin for buffers held by the backend |
| `max_data_per_connection` | 32 MiB | BDP + server pipeline for 25 GbE |
| `quantum` | 32 units (a unit = 1 operation or 64 KiB) | 32 metadata operations ≈ 2 READs of 1 MiB per round |
| `write_timeout` | 30 s | as `sk_sndtimeo` in knfsd |

Example: 64 workers, pool 1024 × 1 MiB. `data_budget` = 896 MiB, `max_pending` = 256,
`max_in_flight` = 256, 1024 connections. Worst case for the memory of parsed requests:
256 × ~0.6 KiB plus 1024 × 256 replies in the write queues (no more than ~75 KiB per connection
without READ data): tens of MiB, not gigabytes.

## 7. Changes by file

| File | Change |
|---|---|
| `nfs-mamont/src/allocator/mod.rs` | `Allocator::granularity()` with a default implementation |
| `nfs-mamont-util/src/allocator/mod.rs` | `granularity()` of `PoolAllocator` = `buffer_size` |
| `nfs-mamont/src/consts/connection.rs` (from #450) | constants of the defaults from §6 |
| `nfs-mamont/src/context.rs` | `with_max_connections`, `with_max_pending`, `with_data_budget`, `with_max_data_per_connection`, `with_scheduler_quantum`, `with_write_timeout`; `Arc<Limits>` with the global budgets; warnings about inconsistent values |
| `nfs-mamont/src/lib.rs` | `max_connections` permit before `accept()`, guard in the connection tasks |
| `nfs-mamont/src/task/connection/mod.rs` | `BudgetedAllocator`, flow registration in `Scheduler`, `Budget` with bytes |
| `nfs-mamont/src/task/connection/limit.rs` (from #450) | second counter (bytes) in `Budget`/`Limiter`; `Token` with `bytes` and `global` |
| `nfs-mamont/src/task/connection/budget_alloc.rs` (new) | the allocator wrapper of §4.3 |
| `nfs-mamont/src/task/connection/read.rs` | `max_pending` after parsing, moving the byte charge into the token, `FlowHandle::enqueue` instead of `pool_sender` |
| `nfs-mamont/src/task/connection/write.rs` | `timeout(write_timeout, form_reply)` |
| `nfs-mamont/src/task/global/vfs/scheduler.rs` (new) | flow queues and DRR of §4.4 |
| `nfs-mamont/src/task/global/vfs/mod.rs` | `VfsManager` owns the `Scheduler`; `VfsCommand` gets `flow`, `cost`, the `max_pending` permit; the worker reads `scheduler.next()` |
| `nfs-mamont/src/task/global/mount.rs`, `nlm/nlm_task.rs` | commands carry the `max_pending` permit |

The public API of `Vfs`, `Allocator::allocate` and `Buffer` does not change; user backends and
allocators keep compiling.

## 8. Phases

1. **Global budgets on top of #450** (R1, R4): `max_connections`, `max_pending`,
   `BudgetedAllocator` with `granularity()`, bytes in the token, a warning and a counter of
   allocator waits. The VFS queue stays FIFO. About 300–400 lines with tests. Already closes
   §4.5/§4.6 of the report and gives an upper bound that does not depend on the number of
   connections.
2. **DRR dispatcher** (R2, R3): `scheduler.rs`, replacing the channel in `VfsManager`, the cost
   model, property tests (§10). Expected result: a light client with limit 256 gets the latency
   it gets today with limit 64, the heavy one loses nothing.
3. **Polish**: `write_timeout`; flow key = `ClientId`; cost by the measured backend time with a
   deficit correction after `dispatch` (an estimate at pick time, corrected by the fact);
   READ buffer allocation at dispatch time (§11); flow weights (quantum per flow) for priorities
   by export or address.

## 9. Alternatives and why not

- **A bounded VFS channel only** (#134, #219). Gives a global bound cheaply, and waiting in
  `send` one per connection is the same round robin by request count. But the service order
  stays FIFO: the latency of a light client ≈ channel depth / throughput (≈ 4 service times with
  `4 × vfs_concurrency`), there is no request cost and no bytes. Acceptable as phase 1, but DRR
  replaces it anyway.
- **Static partitioning** (pool / N connections, workers / N). Contradicts R3: the share of an
  idle client is wasted.
- **A worker pool per connection**. Not work-conserving and multiplies tasks; DRR gives the same
  with a shared pool.
- **NFS3ERR_JUKEBOX as flow control**. The Linux client retries after 5 s
  (`NFS_JUKEBOX_RETRY_TIME`), and for WRITE the data still has to be read from the socket. Only
  fit as a shedding valve when a queue deadline is exceeded, not as the normal mechanism.
- **Per-connection accounting inside the allocator** (report §6.1c). `Allocator` is implemented
  by the user; the accounting must live in the server and work with any allocator.

## 10. Tests and measurements

- Unit tests of `Scheduler`: two backlogged flows get equal cost sums over N rounds; a single
  active flow gets everything; a flow with an empty queue banks no credit; a flow with a
  request more expensive than `quantum` is served over several rounds; closing returns `None`
  after draining.
- Budget tests: round robin between waiting connections; release when a command is destroyed;
  `max_pending` is not held by an idle connection.
- Loopback tests at the connection level (as `task/connection/tests/pipeline.rs` in #450): an
  allocator with one buffer and a `data_budget` of one buffer: the second READ waits in the
  budget while `allocate()` never waits (the `allocator_waits` counter = 0); `write_timeout`
  closes a non-reading client.
- Benchmarks with the stand of the report (`vyar/mock_vfs`, `nfs-bench`, `slow-reader`): repeat
  §4.1, §4.2, §4.6, §4.7 and add "1000 connections" and "fast + slow link". Criteria: §4.1/§4.2
  within noise; §4.6 victim ≥ 55k ops/s; §4.7 light p50 ≤ 1.5 ms with the heavy one ≥ 55k ops/s.

## 11. Open questions

- **Flow key.** Connection or client (`nconnect`)? Depends on #327/#419. Start with the
  connection and move to the client once `ClientId` exists.
- **Several backends** (#351). A slow backend can take all the workers; `max_pending` and DRR do
  not see it. Options: a second DRR level by backend, or a worker ceiling per backend.
- **READ buffer allocation at dispatch time.** Today the READ buffer is taken at parse time
  (`adapter_for_read`), and the request holds it for the whole wait in the queue. If it is
  allocated in the worker, and the scheduler skips a READ whose bytes are not available
  (backfill), the pool only needs `vfs_concurrency × read_max` plus the replies in the write
  queues. This changes the decision "the allocator is not in the VFS task", so it is put into
  phase 3.
- **Serial MOUNT/NLM tasks** (#439). They are under the same permits, but a blocking NLM LOCK in
  one task is a separate head-of-line that this proposal does not solve.
- **Metrics** (#374). The budgets should expose usage, peak, the number and the time of waits;
  the allocator `allocator_waits`; the scheduler the queue depths per flow.
