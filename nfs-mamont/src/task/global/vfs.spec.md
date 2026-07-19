<!-- SPEC_HASH: 63ce4ed0b360f94d0c8b3096961ce32dbc54db7ec09c467ec4e8e466c08d3114 -->
# Module Specification

Module: nfs_mamont::task::global::vfs
Rust File: src/task/global/vfs.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`async_channel`**: Used to create an unbounded Multi-Producer, Multi-Consumer (MPMC) channel. This channel serves as the work queue where the connection handlers submit parsed NFS requests and the worker tasks consume them.
- **`crate::allocator`**: Used to obtain memory buffers for `READ` operations. The `Allocator` trait is invoked to pre-allocate memory before calling the VFS backend, ensuring that the server's memory limits are enforced before I/O begins.
- **`crate::parser`**: Used to define the input payload for the workers. `NfsArgWrapper` contains the parsed RPC arguments and the header (including the XID) necessary to execute the request and route the response.
- **`crate::task`**: Used to define the response type. `ProcReply` is the structure that workers send back to the connection layer, containing the result of the VFS operation.
- **`crate::vfs`**: Used to define the interface for the filesystem backend. The `Vfs` trait aggregates all NFS procedures (e.g., `Read`, `Write`, `Lookup`) that the workers execute. `NfsRes` is the enum wrapping the results of these procedures.
- **`tokio`**: Used to spawn the worker tasks (`tokio::spawn`). The module assumes a Tokio runtime is active.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a concurrent execution pool for NFSv3 procedures, decoupling request reception (network I/O) from request execution (disk I/O and filesystem logic).
- To manage the lifecycle of worker tasks that compete for work from a shared queue.
- To handle the specific memory allocation requirements of the `READ` procedure by interacting with the server's allocator before invoking the VFS backend.

Inputs:
- **`num: NonZeroUsize`**: The number of worker tasks to spawn.
- **`backend: Arc<V>`**: A shared reference to the filesystem implementation.
- **`allocator: Arc<A>`**: A shared reference to the memory allocator.

Outputs:
- **`VfsPool<B>`**: A handle containing a `Sender` that can be cloned and distributed to connection handlers to submit work.

Steps:
1. **Initialization (`VfsPool::new`)**:
 - Creates an unbounded `async_channel`.
 - Spawns `num` instances of `VfsTask` using `tokio::spawn`.
 - Each task receives a clone of the channel's `Receiver` and clones of the `backend` and `allocator`.
 - Returns the `VfsPool` wrapping the channel's `Sender`.
2. **Work Submission**:
 - External modules (e.g., connection handlers) clone the `Sender` from `VfsPool` and send `VfsCommand` tuples `(NfsArgWrapper, Sender<ProcReply>)`.
3. **Work Execution (`VfsTask::run`)**:
 - The worker loops, receiving commands from the `Receiver`.
 - It extracts the RPC header and the `NfsArguments` from the wrapper.
 - It matches on the `NfsArguments` variant:
 - **For `Read`**: If `count` is 0, it uses `Buffer::empty()`. Otherwise, it calls `allocator.allocate()`. If allocation fails, it constructs an error result immediately. If successful, it calls `backend.read()` with the allocated buffer.
 - **For other procedures**: It calls the corresponding method on `backend` (e.g., `backend.write()`, `backend.lookup()`).
 - It wraps the backend result in `NfsRes`.
 - It logs any errors extracted from the result.
 - It constructs a `ProcReply` containing the original `xid` and the result.
 - It sends the `ProcReply` via the `Sender` provided in the command.
4. **Shutdown (`VfsPool::drop`)**:
 - When the pool is dropped, the `Sender` is closed.
 - Workers drain the remaining messages from the channel and then exit the loop once `recv()` returns an error.

Edge Cases:
- **Zero-Length Read**: Handled explicitly by returning `Buffer::empty()` without invoking the allocator.
- **Allocator Exhaustion**: If `allocator.allocate()` returns `None`, the worker returns a `vfs::Error::TooSmall` to the client instead of panicking or blocking indefinitely.
- **Closed Writer**: If the `Sender` provided in the command (for the reply) is closed (e.g., the client disconnected), `tx.send()` fails, and the worker logs a warning but continues processing other commands.

Complexity:
- **Time**: Dominated by the latency of the `backend` operations (disk I/O) and the allocator.
- **Space**: The channel is unbounded, so memory usage grows if producers outpace consumers. The number of active tasks is fixed by `num`.

Determinism:
- **Non-deterministic**. The order of execution depends on the scheduler and the speed of the workers. However, the system ensures that every command received is eventually processed exactly once (unless the pool is dropped prematurely).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
 - **`Vfs<B>` Trait**: The module relies on the `Vfs` trait as a polymorphic interface to the filesystem. The worker pool does not know the specific filesystem implementation; it simply dispatches calls to the methods defined in this trait (e.g., `read`, `write`).
 - **`NfsRes<B>` Enum**: The module uses this enum to unify the return types of all VFS operations. This allows the worker to have a single code path for sending replies, regardless of which specific NFS procedure was executed.

- **From `nfs_mamont::allocator`**:
 - **`Allocator` Trait**: The module uses this to decouple memory management from the VFS logic. Specifically, for `READ` operations, the pool acts as the "client" of the allocator, requesting a buffer of a specific size and passing ownership of that buffer to the VFS backend. This ensures that memory limits are enforced at the pool level.

- **From `nfs_mamont::parser`**:
 - **`NfsArgWrapper<B>`**: This struct serves as the standardized "command packet" for the queue. It bundles the RPC header (needed for the reply XID) with the parsed arguments (needed for the backend call), allowing the worker to have everything it needs in a single channel message.

- **From `nfs_mamont::task`**:
 - **`ProcReply<B>`**: This is the standardized "response packet". The worker constructs this struct to send the result back through the reply channel, ensuring the connection layer receives a type-safe, serializable result.

---

## 4. Data Model

Entities:
- **`VfsPool<B>`**: A handle to the worker pool. Contains a `Sender<VfsCommand<B>>`.
- **`VfsTask<A, V, B>`**: An asynchronous worker task. Contains references to the `backend` (VFS), `allocator`, and the command `Receiver`.
- **`VfsCommand<B>`**: A type alias for `(NfsArgWrapper<B>, Sender<ProcReply<B>>)`. Represents a unit of work containing the request arguments and a channel to send the response.

Relations:
- **`VfsPool` → `VfsTask` (1:N)**: The pool creates and manages the lifecycle of N tasks.
- **`VfsTask` → `Vfs` (1:1)**: Each task holds a reference to a shared VFS backend.
- **`VfsTask` → `Allocator` (1:1)**: Each task holds a reference to a shared allocator.
- **`VfsCommand` → `NfsArgWrapper` (Composition)**: The command contains the parsed request.
- **`VfsCommand` → `Sender<ProcReply>` (Composition)**: The command contains the channel to send the result back.

Global Invariants:
- **Channel Consistency**: All workers share a clone of the same `Receiver`. When the `Sender` in `VfsPool` is dropped, all workers will eventually observe a channel close error and terminate.
- **Reply Routing**: The `xid` in the `ProcReply` sent by the worker must match the `xid` in the `NfsArgWrapper` received in the command.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Errors returned by the VFS backend (e.g., `NoEntry`, `IO`, `Permission`).
- **`vfs::Error::TooSmall`**: A specific error generated by the pool if the allocator fails to provide a buffer for a `READ` operation.

Error Propagation Strategy:
- **Result Wrapping**: Errors from the backend are wrapped in the `Err` variant of the specific procedure's result type (e.g., `NfsRes::Read(Err(...))`).
- **Logging**: The module inspects the `NfsRes` and logs errors using the `tracing` crate before sending the reply.
- **Reply Transmission**: Errors are sent back to the caller as `ProcReply { proc_result: Ok(ProcResult::Nfs3(Box::new(NfsRes::Read(Err(...))))) }`. Note that the `proc_result` is `Ok`, indicating the RPC dispatch succeeded, but the NFS procedure itself failed.

Recoverability:
- **Recoverable**: Errors in VFS operations do not crash the worker. They are logged, packaged into a reply, and sent to the client. The worker then continues to the next command.
- **Allocator Failure**: If the allocator is exhausted, the worker returns a `TooSmall` error to the client, effectively failing the specific read request but keeping the server running.

Panics:
- **Allowed**: Yes.
- **Conditions**:
 - `VfsTask::spawn` panics if called outside of a Tokio runtime.
 - `unwrap()` on `NonZeroUsize::new` in the `Read` branch (though logically safe given the `count > 0` check).

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **offload the execution of filesystem operations from the network I/O threads**, ensuring that slow disk operations or complex filesystem logic do not block the handling of incoming network packets. In a high-performance NFS server, it is critical to keep the network layer responsive to accept new connections and receive requests, even if the server is currently busy reading from disk.

The system contains a **producer-consumer architecture** where the connection handlers (producers) parse incoming RPC messages and push them onto a queue. The `VfsPool` (consumer) maintains a fixed number of worker tasks that pull messages from this queue, execute the corresponding VFS operations, and send the results back.

A typical usage scenario of the system involves the main server loop initializing a `VfsPool` with a specific number of workers (e.g., equal to the number of CPU cores). When a client sends a `READ` request:
1. The connection handler parses the bytes into a `NfsArgWrapper`.
2. It creates a oneshot channel for the reply and sends the `(Wrapper, ReplySender)` tuple to the `VfsPool`.
3. An idle worker in the pool receives the tuple.
4. The worker checks the arguments. If it's a `READ`, it requests a buffer from the `Allocator`.
5. The worker calls `backend.read()`, passing the buffer.
6. The backend fills the buffer (potentially blocking on disk I/O).
7. The worker wraps the result in a `ProcReply` and sends it back via the `ReplySender`.
8. The connection handler, which was waiting on the reply channel, receives the result and serializes it to the network socket.

Inside the system, the following things happen and they use this module:
- **Concurrency Control**: By using a fixed-size pool, the system limits the amount of concurrent I/O and CPU load on the VFS backend. If 1000 requests arrive simultaneously, they are queued in the channel, but only `num` workers are active at any time, preventing resource exhaustion.
- **Memory Management Integration**: The pool acts as the bridge between the `allocator` module and the `vfs` module. It ensures that memory for `READ` operations is allocated *before* the VFS is called, enforcing the server's memory limits at the entry point of the execution pipeline.
- **Error Isolation**: If a specific VFS operation panics or encounters a fatal error, it only affects the specific worker task (which may be restarted by the runtime depending on configuration), but the pool structure and other workers remain intact, preserving the availability of the server for other requests.

Without this module, the server would likely execute VFS operations directly in the connection tasks. This would couple network latency to disk latency, reducing throughput and increasing tail latency for all clients. The `VfsPool` is essential for achieving high concurrency and isolating the filesystem subsystem from the network subsystem.