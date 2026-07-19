<!-- SPEC_HASH: 63ce4ed0b360f94d0c8b3096961ce32dbc54db7ec09c467ec4e8e466c08d3114 -->
# Module Specification

Module: nfs_mamont::task::global::vfs
Rust File: src/task/global/vfs.rs

---

## 1. Dependencies

From the provided context and dependency facts:

- **`async_channel`**: Used to create an unbounded Multi-Producer, Single-Consumer (MPSC) channel (`Sender`/`Receiver`). This facilitates the work queue where parsed NFS requests are enqueued and worker tasks compete to process them.
- **`tokio`**: Specifically `tokio::spawn`, used to launch `VfsTask` workers as asynchronous tasks onto the Tokio runtime. This allows the pool to execute blocking or long-running filesystem operations concurrently without stalling the main event loop.
- **`tracing`**: Used for instrumentation. `error!` is used to log filesystem operation failures (extracted from `NfsRes`), and `warn!` is used to signal when the downstream writer task has terminated, causing the connection pipeline to close.
- **`crate::allocator`**: Provides the `Allocator` and `Buffer` traits. The module uses the `Allocator` to request memory buffers for `READ` operations, ensuring that memory usage is controlled by the server's allocation policy rather than the VFS backend.
- **`crate::parser`**: Provides `NfsArgWrapper` and `NfsArguments`. These types represent the parsed input data (RPC headers and specific NFS procedure arguments) that the pool processes.
- **`crate::vfs`**: Provides the `Vfs` trait and `NfsRes` enum. The `Vfs` trait defines the interface for filesystem operations (e.g., `read`, `write`, `lookup`) that the pool dispatches to. `NfsRes` is the unified result type for these operations.
- **`crate::task`**: Provides `ProcReply` and `ProcResult`. These are used to wrap the `NfsRes` into a structure compatible with the downstream writer pipeline, including the XID for correlation.

---

## 2. Mechanics

**Intent:**
The module implements a fixed-size worker pool pattern to decouple the reception of NFS requests from their execution. Its primary goal is to manage concurrency by distributing filesystem operations across a set number of asynchronous tasks, ensuring that slow or blocking I/O in the backend does not stall the entire server, while integrating strictly with the server's memory allocation strategy for read operations.

**Inputs:**
- **Configuration (`VfsPool::new`)**:
  - `num`: Number of worker tasks to spawn (`NonZeroUsize`).
  - `backend`: Shared reference-counted pointer to the filesystem implementation (`Arc<V>`).
  - `allocator`: Shared reference-counted pointer to the memory allocator (`Arc<A>`).
- **Runtime Request (`VfsCommand`)**:
  - `command`: Parsed NFS arguments (`NfsArgWrapper`).
  - `tx`: Channel sender to return the result (`Sender<ProcReply<B>>`).

**Outputs:**
- **Pool Instance**: A `VfsPool` struct holding a `Sender` to enqueue work.
- **Replies**: `ProcReply` messages sent asynchronously to the provided `tx` channel.

**Steps:**

1.  **Initialization (`VfsPool::new`)**:
    - Creates an unbounded `async_channel` for distributing commands.
    - Iterates `num` times. In each iteration, it clones the `Receiver` and the shared `backend` and `allocator` references.
    - Instantiates a `VfsTask` with these references and calls `spawn`, which moves the task onto the Tokio runtime.
    - Returns the `VfsPool` wrapping the `Sender`.

2.  **Task Execution (`VfsTask::run`)**:
    - Enters an infinite loop awaiting commands from the `Receiver`.
    - **Dispatch**: Upon receiving a `(command, tx)` tuple, it destructures the `NfsArguments` enum.
    - **Read Handling**: For `NfsArguments::Read`:
        - If `args.count` is 0, it uses `Buffer::empty()` to avoid allocation.
        - Otherwise, it calls `allocator.allocate` with the requested size.
        - If allocation fails (returns `None`), it constructs a `vfs::read::Fail` with `vfs::Error::TooSmall`.
        - If successful, it passes the allocated buffer to `backend.read`.
    - **Generic Handling**: For all other procedures, it calls the corresponding method on `backend` (e.g., `backend.write`, `backend.lookup`).
    - **Logging**: It inspects the resulting `NfsRes`. If the variant is an `Err`, it extracts the `vfs::Error` and logs it with the XID and procedure name.
    - **Response**: It wraps the `NfsRes` in `ProcResult::Nfs3` and then `ProcReply`, preserving the XID.
    - **Reply Transmission**: It attempts to send the reply via `tx`. If the send fails (indicating the receiver/writer task is dropped), it logs a warning.

3.  **Shutdown (`VfsPool::drop`)**:
    - When the pool is dropped, the `Sender` is closed.
    - The `Receiver` in all workers will eventually return `Err` (Closed) after draining the queue.
    - The `run` loop terminates, and the worker tasks complete.

**Edge Cases:**
- **Zero-Length Read**: Handled explicitly by returning `Buffer::empty()` without invoking the allocator.
- **Allocation Failure**: Handled by converting the `None` from `allocate` into a specific NFS error (`TooSmall`) rather than panicking.
- **Writer Disconnection**: If the `tx.send` fails, the worker logs the event and continues processing other commands (if any) until the channel closes.

**Complexity:**
- **Time**:
  - `new`: O(N) to spawn tasks.
  - `run`: O(1) for dispatch; execution time depends entirely on the `Vfs` backend implementation and `Allocator` speed.
- **Space**: O(Q) where Q is the size of the unbounded channel queue. If producers outpace workers, memory usage grows indefinitely.

**Determinism:**
- **Non-deterministic**. The order of execution depends on the scheduler and which worker acquires the command from the channel first. The timing of operations depends on the backend I/O latency.

---

## 3. Dependency Mechanics

- **`Allocator` (from `nfs_mamont::allocator`)**:
    - **Bounded Memory**: The module relies on the `Allocator` to enforce memory limits for read data. By calling `allocate` within the worker, the pool ensures that a request is only processed if memory is available, naturally applying backpressure.
    - **Buffer Lifecycle**: The module assumes the `Buffer` returned by the allocator can be passed directly to the `Vfs::read` method and subsequently moved into the `ProcReply`.

- **`Vfs` (from `nfs_mamont::vfs`)**:
    - **Async Interface**: The module treats the `Vfs` trait as a collection of asynchronous methods. It blindly awaits the results, meaning the pool's throughput is directly tied to the concurrency of the `Vfs` implementation.
    - **Error Mapping**: The module assumes `Vfs` methods return `Result` types compatible with `NfsRes`. It performs pattern matching on these results to extract domain-specific errors for logging.

- **`NfsArguments` (from `nfs_mamont::parser`)**:
    - **Dispatch Key**: The module uses the enum variant of `NfsArguments` as the primary key to determine which `Vfs` method to call. This creates a tight coupling between the parser's definition of procedures and the pool's dispatch logic.

---

## 4. Data Model

**Entities:**
- **`VfsPool`**: The public handle to the pool. Owns the `Sender` end of the command channel.
- **`VfsTask`**: The worker logic. Holds references to the `Vfs` backend, `Allocator`, and the `Receiver`.
- **`VfsCommand`**: A tuple alias `(NfsArgWrapper<B>, Sender<ProcReply<B>>)`. Represents a unit of work containing the input and the destination for the output.

**Relations:**
- **`VfsPool` → `VfsCommandSender` (1:1)**: The pool owns the sender used to enqueue jobs.
- **`VfsTask` → `VfsCommandReceiver` (N:1)**: Multiple workers share a reference to the same receiver (cloned during pool creation).
- **`VfsTask` → `Vfs` (N:1)**: All workers share a reference to the single backend instance.
- **`VfsTask` → `Allocator` (N:1)**: All workers share a reference to the single allocator instance.

**Global Invariants:**
- **Worker Count**: The number of active `VfsTask`s running on the runtime is exactly `num` until the `VfsPool` is dropped.
- **Channel Consistency**: The `Sender` in `VfsPool` and the `Receiver`s in `VfsTask`s belong to the same channel instance created in `new`.

---

## 5. Error Model

**Error Types:**
- **`vfs::Error`**: Domain errors returned by the `Vfs` backend (e.g., `NoEntry`, `IO`, `Permission`).
- **`vfs::read::Fail`**: Specific error type for read operations, constructed locally if allocation fails.

**Error Propagation Strategy:**
- **Logging**: Errors are intercepted in the `run` loop. If the `NfsRes` is an `Err` variant, the inner `vfs::Error` is extracted and logged via `tracing::error!`.
- **Client Notification**: Errors are wrapped in the `Ok` variant of `ProcReply` (inside `ProcResult::Nfs3`). The system distinguishes between "operation failed" (logical error) and "system failure" (transport error).

**Recoverability:**
- **Operation Errors**: Fully recoverable. The worker logs the error, sends the failure response to the client, and immediately loops to process the next command.
- **Allocation Failures**: Recoverable. Treated as an operation error (`TooSmall`), preventing the worker from crashing.

**Panics:**
- **Allowed**: Yes.
- **Conditions**:
    - In `VfsTask::spawn`: If called outside of a Tokio runtime context.
    - In `VfsTask::run`: If `NonZeroUsize::new(args.count as usize).unwrap()` panics. This is theoretically unreachable because the check `args.count == 0` guards the `unwrap`, but it represents a logical invariant.

---

## 6. Traits

The module implements the following traits:
- **`Drop`** for `VfsPool`: Closes the command channel to signal shutdown to workers.

The module uses the following external traits:
- **`Allocator`** (from `crate::allocator`): To allocate buffers for read operations.
- **`Buffer`** (from `crate::allocator`): To handle empty buffers and read data.
- **`Vfs`** (from `crate::vfs`): To execute filesystem logic.

---

## 7. Overview

This module is used in order to **manage the execution layer of the NFS server**, providing a dedicated thread pool for filesystem operations that isolates the I/O-bound and potentially blocking backend logic from the network handling layer.

This system contains a **concurrency control mechanism** that bridges the asynchronous network pipeline (parser/writer) with the synchronous or asynchronous filesystem backend (`Vfs`). It ensures that while the server can handle many connections, the actual filesystem work is parallelized across a fixed number of workers (`num`), preventing resource exhaustion.

A typical usage scenario of the system involves a client requesting to read a file. The network layer receives the bytes, parses them into `NfsArguments::Read`, and wraps them in a `VfsCommand`. This command is sent to the `VfsPool`. One of the idle `VfsTask` workers picks up the command. It checks the read count, requests a buffer of that size from the `Allocator` (which enforces global memory limits), and then calls `backend.read`. Once the backend fills the buffer, the worker wraps the result in `ProcReply` and sends it back to the specific channel associated with that request, which routes it to the correct network writer task.

Inside the system, the following things happen and they use this module:
1.  **Backpressure Propagation**: By relying on the `Allocator` to provide buffers for reads, the pool propagates memory pressure. If the server is low on memory, `allocate` returns `None`, and the worker immediately returns a "TooSmall" error to the client without touching the disk.
2.  **Graceful Degradation**: If the network writer task crashes or disconnects (e.g., the client hangs up), the `tx.send` in the worker will fail. The worker detects this, logs a warning, and continues processing other requests, ensuring that a single bad connection does not stall the worker pool.
3.  **Operation Isolation**: The pool treats every NFS operation as an independent unit of work. Errors in one operation (e.g., permission denied) are logged and converted into standard NFS error replies, leaving the worker ready for the next task immediately.

The critical aspect of this module is the **integration of the allocator into the request processing loop**. Instead of letting the VFS backend allocate memory arbitrarily, the pool enforces the use of the centralized `Allocator`, which is essential for maintaining predictable memory usage in a high-performance server environment.