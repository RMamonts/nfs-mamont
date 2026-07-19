<!-- SPEC_HASH: 138c62cfb0e459625697401184fb13e5f8ca6dcbc5960d4994d3e9f766e2a01d -->
# Module Specification

Module: nfs_mamont::task::global::nlm
Rust File: src/task/global/nlm.rs

---

## 1. Dependencies

From the provided context and dependency facts:

- **`async_channel`**: Used to create an unbounded multi-producer, single-consumer (MPSC) channel. This channel facilitates the communication between connection read tasks (producers) and the `NlmTask` (consumer).
- **`tokio`**: Used to spawn the `NlmTask` onto the asynchronous runtime via `tokio::spawn`. This allows the dispatcher to run concurrently with other server tasks without blocking the main thread.
- **`crate::nlm::Nlm`**: The core trait defining the NLM v4 service logic (Lock, Unlock, Test, Cancel). The `NlmTask` acts as a generic driver, delegating the actual lock management to an implementation of this trait.
- **`crate::task`**: Provides the `ProcReply` and `ProcResult` types. These are used to wrap the results of NLM operations into a standardized format that can be sent back to the network write tasks.
- **`crate::parser`**: Provides `NlmArgWrapper` and `NlmArguments`. These types represent the parsed input data received from the network, which the `NlmTask` unpacks and processes.
- **`crate::allocator::Buffer`**: A generic constraint `B` used within `ProcReply`. While NLM results primarily consist of status codes, the generic buffer type allows the reply structure to integrate with the broader memory management system of the server.

---

## 2. Mechanics

**Intent:**
The module implements an asynchronous task dispatcher for the Network Lock Manager (NLM) v4 protocol. Its purpose is to decouple the network I/O (reading requests and writing responses) from the execution of lock management logic. By centralizing NLM procedure calls in a single background task, it serializes access to the shared `Nlm` service, ensuring that lock state modifications occur in a deterministic order without requiring explicit locking primitives within the service implementation itself.

**Inputs:**
- **Initialization (`new`)**:
  - `nlm_service`: An `Arc<N>` implementing the `Nlm` trait, containing the business logic for lock management.
- **Runtime (`receiver.recv`)**:
  - `NlmCommand`: A message struct containing:
    - `result_tx`: A channel sender to return the result.
    - `args`: A `NlmArgWrapper` containing the RPC header and the specific procedure arguments (Lock, Unlock, etc.).

**Outputs:**
- **Initialization**: A tuple of `(NlmTask, Sender<NlmCommand<B>>)`. The sender is distributed to read tasks to submit commands.
- **Runtime**: `ProcReply<B>` messages sent asynchronously via the `result_tx` channels contained in incoming commands.

**Steps:**

1. **Initialization (`NlmTask::new`)**:
   - Creates an unbounded `async_channel` for `NlmCommand<B>`.
   - Instantiates `NlmTask`, storing the `receiver` end of the channel and the shared `nlm_service`.
   - Returns the task instance and the `sender` end of the channel.

2. **Spawning (`NlmTask::spawn`)**:
   - Takes ownership of `self`.
   - Uses `tokio::spawn` to schedule the `self.run()` future on the Tokio runtime.
   - **Panic Condition**: Panics if called outside a Tokio runtime context.

3. **Event Loop (`NlmTask::run`)**:
   - Enters an infinite `while let Ok(command) = receiver.recv().await` loop.
   - **Destructuring**: Unpacks `NlmCommand` into `result_tx`, `header`, and `proc`.
   - **Dispatching**: Matches on the `proc` enum variant:
     - `Null`: Returns `NlmRes::Null`.
     - `Lock`: Calls `nlm_service.lock(args).await`. Wraps result in `NlmRes::Lock`.
     - `Unlock`: Calls `nlm_service.unlock(args).await`. Wraps result in `NlmRes::Unlock`.
     - `Test`: Calls `nlm_service.test(args).await`. Wraps result in `NlmRes::Test`.
     - `Cancel`: Calls `nlm_service.cancel(args).await`. Wraps result in `NlmRes::Cancel`.
   - **Response Construction**: Creates a `ProcReply` struct containing the original `xid` (from the header) and the `NlmRes` wrapped in `Ok(ProcResult::Nlm4(...))`.
   - **Reply Transmission**: Attempts to send the `ProcReply` via `result_tx.send().await`.
   - **Error Handling**: The result of `send` is explicitly ignored (`let _ = ...`). If the receiver (write task) has dropped the channel, the send fails silently, and the loop continues.

**Edge Cases:**
- **Disconnected Write Task**: If the `result_tx` channel is closed (e.g., the client disconnected), the `send` operation fails. The task catches this failure (via `let _ =`), logs the reply queued (if successful), and continues processing the next command without crashing.
- **Service Panic**: If the underlying `nlm_service` method panics, the `NlmTask` will terminate, as the panic propagates through the `await` point.

**Complexity:**
- **Time**: Per-command latency is determined by the execution time of the specific `Nlm` service method (e.g., `lock`) plus the overhead of channel passing.
- **Space**: O(1) stack space per command execution. The unbounded channel grows indefinitely if backpressure is not applied by producers, potentially leading to unbounded memory consumption in high-load scenarios.

**Determinism:**
- **Non-deterministic** regarding execution order relative to other tasks, but **deterministic** regarding the processing of commands within the task (FIFO order guaranteed by the channel).

---

## 3. Dependency Mechanics

- **`Nlm` Trait (from `nfs_mamont::nlm`)**:
  - **Service Abstraction**: The module relies on the `Nlm` trait to perform the actual lock operations. The `NlmTask` is agnostic to the specific implementation; it simply calls the methods defined in the trait (`lock`, `unlock`, `test`, `cancel`). This allows the dispatcher to be tested with mock services or swapped for different locking strategies.
  - **Result Types**: The module assumes the trait methods return specific result types (e.g., `Nlm4LockRes`) which are compatible with the variants of the `NlmRes` enum.

- **`ProcReply` and `ProcResult` (from `nfs_mamont::task`)**:
  - **Response Envelope**: The module uses `ProcReply` to standardize the output. It maps the low-level `NlmRes` into `ProcResult::Nlm4`, preserving the generic `Buffer` type `B`. This allows the write task to handle replies from different protocols (NFS, MOUNT, NLM) uniformly.

- **`NlmArgWrapper` (from `nfs_mamont::parser`)**:
  - **Input Decoupling**: The module consumes `NlmArgWrapper`, which encapsulates the RPC header (containing the transaction ID `xid`) and the procedure arguments. This allows the dispatcher to extract the `xid` for correlation without needing to understand the raw bytes of the RPC message.

---

## 4. Data Model

**Entities:**
- **`NlmCommand<B>`**: A data transfer object representing a request. It holds the mechanism to reply (`result_tx`) and the payload (`args`).
- **`NlmTask<B, N>`**: The actor entity. It holds the state necessary to process commands: the link to the service (`nlm_service`) and the input queue (`receiver`).

**Relations:**
- **`NlmTask` → `NlmCommand` (1:N)**: The task consumes commands from the channel. Many commands may be queued in the channel buffer.
- **`NlmCommand` → `Sender<ProcReply<B>>` (1:1)**: Each command carries a dedicated channel sender to route the response back to the specific context that issued the request.

**Global Invariants:**
- **XID Correlation**: The `xid` extracted from the `NlmArgWrapper` header must be passed into the `ProcReply` to ensure the client can match the response to the request.
- **Sequential Processing**: Only one command is processed at a time by the `NlmTask` loop. This ensures that calls to the `Nlm` service are effectively serialized.

---

## 5. Error Model

**Error Types:**
- None defined explicitly within this module.

**Error Propagation Strategy:**
- **Silent Ignorance**: Send errors on `result_tx` are explicitly ignored. This treats a disconnected client as a "fire and forget" event for the reply.
- **Panic Propagation**: Panics originating from the `nlm_service` or the channel operations will propagate and crash the task.

**Recoverability:**
- **Send Failures**: Recoverable. The task continues to the next command.
- **Service Panics**: Not recoverable. The task terminates, and the `receiver` is dropped. Any subsequent sends to the `Sender` held by other tasks will fail (likely causing those tasks to log errors or terminate depending on their implementation).

**Panics:**
- **Allowed**: Yes.
- **Conditions**:
  - If `tokio::spawn` is called outside of a Tokio runtime.
  - If the `nlm_service` method panics during execution.

---

## 6. Traits

The module does not define or implement any public traits. It acts as a consumer of the `Nlm` trait defined in `nfs_mamont::nlm`.

---

## 7. Overview

This module is used in order to **provide a dedicated, asynchronous execution context for NLM protocol operations**, effectively acting as a dispatcher that serializes requests to the lock manager. It solves the problem of concurrency control by ensuring that all modifications to the lock state occur within a single logical thread of execution (the `NlmTask`), thereby preventing race conditions without requiring complex locking logic within the lock service implementation itself.

This system contains a **centralized dispatcher task** that bridges the gap between the network layer (parsers and serializers) and the core business logic (the `Nlm` service). It utilizes an unbounded channel to buffer incoming requests, allowing the system to handle bursts of NLM traffic without immediately blocking the connection read tasks.

A typical usage scenario of the system involves a client requesting a file lock. The connection read task parses the RPC bytes into a `NlmArgWrapper`, constructs a `NlmCommand` containing a reply channel, and sends it to the `NlmTask`. The `NlmTask` receives the command, extracts the arguments, and invokes the `lock` method on the shared `Nlm` service. Once the service returns a result (e.g., `Granted`), the `NlmTask` wraps it in a `ProcReply` and sends it back through the reply channel. The connection write task, waiting on this channel, receives the result and serializes it back to the client.

Inside the system, the following things happen and they use this module:
1.  **Request Decoupling**: The network layer (read tasks) offloads the processing of NLM requests to this module. This allows the read tasks to continue reading data from the socket while the `NlmTask` handles the potentially slow lock management logic.
2.  **State Serialization**: By funneling all NLM requests through a single `NlmTask` instance, the system guarantees that the `Nlm` service never processes two lock requests simultaneously. This is critical for maintaining the integrity of the lock database.
3.  **Response Correlation**: The module ensures that the Transaction ID (`xid`) from the request is preserved and attached to the response. This is vital for the RPC layer to match asynchronous replies to the correct outstanding requests on the client side.