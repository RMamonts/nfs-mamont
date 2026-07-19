<!-- SPEC_HASH: 138c62cfb0e459625697401184fb13e5f8ca6dcbc5960d4994d3e9f766e2a01d -->
# Module Specification

Module: nfs_mamont::task::global::nlm
Rust File: src/task/global/nlm.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`async_channel::{Receiver, Sender}`**: Used to create an unbounded asynchronous channel (`unbounded`). This channel facilitates the communication between the connection read tasks (producers) and the global NLM task (consumer). `Sender` is part of `NlmCommand` to send replies back to the write tasks.
- **`std::sync::Arc`**: Used to wrap the `Nlm` service implementation (`nlm_service`). This allows the service to be shared safely and cloned if necessary, ensuring that the global task maintains ownership or access to the locking logic without transferring ownership of the service itself.
- **`tokio`**: Used via `tokio::spawn` to run the `NlmTask` logic asynchronously on the Tokio runtime. This allows the task to process commands concurrently with the rest of the server operations.
- **`tracing`**: Used via the `debug!` macro to log the receipt of commands, the specific procedure being invoked (e.g., `NLM LOCK`), and the queuing of replies. This is crucial for observability and debugging the flow of locking requests.
- **`crate::allocator::Buffer`**: Used as a generic constraint `B: Buffer` for `NlmCommand`. This ensures that any buffers involved in the command structure (though `NlmArgWrapper` itself doesn't directly hold a buffer in this specific file, the surrounding system does) adhere to the server's memory management interface.
- **`crate::nlm::Nlm`**: Used as a trait bound `N: Nlm` for the `NlmTask`. This defines the interface that the background task calls to perform the actual locking operations (`lock`, `unlock`, `test`, `cancel`).
- **`crate::nlm::NlmRes`**: Used to construct the response payload. The task converts the results from the `Nlm` service into `NlmRes` enum variants to be wrapped in a `ProcReply`.
- **`crate::task::{ProcReply, ProcResult}`**: Used to construct the final message sent back to the client. `ProcReply` wraps the result and the transaction ID (`xid`), while `ProcResult::Nlm4` wraps the `NlmRes`.
- **`crate::parser::{NlmArgWrapper, NlmArguments}`**: Used to define the input payload. `NlmCommand` contains `NlmArgWrapper`, which holds the parsed RPC header and the specific procedure arguments (`NlmArguments`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement an "Actor" pattern for the Network Lock Manager (NLM) protocol. By centralizing the execution of NLM procedures in a single background task, the module serializes access to the lock state without requiring explicit mutexes in the service implementation. It decouples the connection handling (parsing/serialization) from the business logic (locking).

Inputs:
- **`nlm_service: Arc<N>`**: A shared reference to the implementation of the NLM protocol logic.
- **`NlmCommand<B>`**: Messages received from the `Receiver` channel. Each command contains the arguments parsed from the network (`args`) and a channel sender (`result_tx`) to route the response back to the specific client connection.

Outputs:
- **`Sender<NlmCommand<B>>`**: Returned by `new`, allowing other tasks (connection readers) to submit NLM requests.
- **`ProcReply<B>`**: Sent asynchronously via the `result_tx` channel contained in the incoming command. This contains the result of the NLM operation.

Steps:
1. **Initialization (`new`)**:
   - Creates an unbounded `async_channel`.
   - Instantiates `NlmTask` holding the `nlm_service` and the `receiver` end of the channel.
   - Returns the task instance and the `sender` end of the channel.

2. **Spawning (`spawn`)**:
   - Takes ownership of `self`.
   - Calls `tokio::spawn` passing `self.run()`.
   - This detaches the task lifecycle from the caller, allowing it to run indefinitely on the runtime.

3. **Event Loop (`run`)**:
   - Enters a `while let Ok(command) = receiver.recv().await` loop.
   - **Receive**: Waits for a `NlmCommand`.
   - **Extract**: Destructures the command into `result_tx` and `args` (which contains `header` and `proc`).
   - **Dispatch**: Matches on the `proc` enum (`NlmArguments`):
     - `Null`: Returns `NlmRes::Null`.
     - `Lock`: Calls `nlm_service.lock(args).await`, wraps result in `NlmRes::Lock`.
     - `Unlock`: Calls `nlm_service.unlock(args).await`, wraps result in `NlmRes::Unlock`.
     - `Test`: Calls `nlm_service.test(args).await`, wraps result in `NlmRes::Test(Box::new(res))`.
     - `Cancel`: Calls `nlm_service.cancel(args).await`, wraps result in `NlmRes::Cancel`.
   - **Respond**: Constructs a `ProcReply` with the original `xid` and the `Ok(ProcResult::Nlm4(...))`.
   - **Send**: Sends the reply via `result_tx.send().await`. Errors during send are ignored (logged as `_`), implying that if the connection write task has died, the NLM task continues processing other commands.

Edge Cases:
- **Channel Disconnection**: If all senders are dropped, `receiver.recv().await` will return an `Err`, causing the `while` loop to terminate and the task to shut down gracefully.
- **Send Failure**: If `result_tx.send` fails (e.g., the receiving write task has crashed), the error is explicitly ignored (`let _ = ...`). The task continues to the next command, ensuring one bad connection doesn't stall the global lock manager.
- **Panics in Service**: If the `nlm_service` method panics, the `run` task will panic and terminate, as there is no `catch_unwind` wrapper.

Complexity:
- **Time**: O(1) for dispatch overhead. The actual time depends on the `nlm_service` implementation (which likely involves I/O or state management).
- **Space**: O(N) where N is the depth of the channel backlog (unbounded). The task itself holds minimal state.

Determinism:
- **Non-deterministic**. The order of command processing depends on the order of arrival via the channel and the scheduling of the asynchronous runtime.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::nlm`**:
  - **`Nlm` Trait**: The module relies on the `Nlm` trait to define the operations it can dispatch. It treats the service as a black box implementing `lock`, `unlock`, `test`, and `cancel`.
  - **`NlmRes` Enum**: The module uses `NlmRes` to wrap the specific return types from the service methods into a single type that can be sent back to the connection layer.

- **From `nfs_mamont::task`**:
  - **`ProcReply` and `ProcResult`**: The module uses these types to standardize the response format. It wraps the `NlmRes` in `ProcResult::Nlm4` and then places that in `ProcReply` along with the `xid`. This ensures the response is compatible with the generic RPC write logic.

- **From `nfs_mamont::parser`**:
  - **`NlmArgWrapper` and `NlmArguments`**: The module consumes these types. `NlmArgWrapper` provides the `xid` needed for the reply, and `NlmArguments` is the enum matched against to determine which service method to call.

---

## 4. Data Model

Entities:
- **`NlmCommand<B: Buffer>`**: A message structure representing a request.
  - `result_tx`: `Sender<ProcReply<B>>` - Channel to send the response.
  - `args`: `NlmArgWrapper` - The parsed procedure arguments and RPC header.
- **`NlmTask<B, N>`**: The actor task.
  - `nlm_service`: `Arc<N>` - The lock manager implementation.
  - `receiver`: `Receiver<NlmCommand<B>>` - The input queue.

Relations:
- **`NlmTask` → `NlmCommand` (1:N)**: The task consumes commands from the receiver.
- **`NlmCommand` → `NlmArgWrapper` (1:1)**: Composition.
- **`NlmCommand` → `Sender` (1:1)**: Composition. The command carries the capability to reply.

Global Invariants:
- **Single Consumer**: The `receiver` in `NlmTask` is the sole consumer of the channel. While `Sender` handles can be cloned and distributed across many connection tasks, only one `NlmTask` instance should hold the `receiver` to ensure serial processing of lock requests.
- **XID Preservation**: The `xid` extracted from `args.header` must be used in the `ProcReply` sent via `result_tx` to ensure the client can match the reply to the request.

## 5. Error Model

Error Types:
- **None defined explicitly**. The module does not define custom error enums.

Error Propagation Strategy:
- **Service Errors**: Errors returned by `nlm_service` methods (e.g., `Nlm4Stats::Denied`) are treated as successful results (`Ok`) wrapped in `NlmRes`. They are not treated as Rust `Err` variants.
- **Channel Errors**: 
  - `receiver.recv()` returning `Err` causes the task loop to exit (graceful shutdown).
  - `result_tx.send()` returning `Err` is caught and ignored (`let _ = ...`), preventing the task from crashing if a connection drops.

Recoverability:
- **Recoverable**: The task is resilient to individual connection failures (dropped `result_tx`). It continues processing the next command in the queue.
- **Terminal**: If the input channel closes (all senders dropped), the task terminates.

Panics:
- **Allowed**: Yes.
- **Conditions**:
  - If `spawn` is called outside of a Tokio runtime.
  - If the `nlm_service` method panics (propagates up to `run`).

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **centralize and serialize the execution of Network Lock Manager (NLM) protocol procedures** within the NFS-Mamont server. In a distributed file system, locking operations are critical for data consistency and require strict ordering to prevent race conditions. This module implements an "Actor" pattern where a single asynchronous task (`NlmTask`) owns the logic for processing all lock requests from all connected clients.

The system contains a **global dispatcher** for NLM requests. Instead of having every connection handler interact directly with a shared, potentially mutex-protected lock state, connection handlers parse the NLM request and wrap it in a `NlmCommand`. This command is sent to the `NlmTask` via an unbounded channel. The `NlmTask` processes these commands sequentially, invoking the appropriate method on the `Nlm` service implementation (e.g., `lock`, `unlock`). Once the service returns a result, the task wraps it in a `ProcReply` and sends it back to the specific connection's write task using the sender provided in the command.

A typical usage scenario of the system involves multiple clients attempting to lock the same file simultaneously.
1. Client A sends a LOCK request. The connection read task parses it and sends a `NlmCommand` to the global `NlmTask`.
2. Client B sends a LOCK request shortly after. Its connection read task also sends a `NlmCommand` to the same `NlmTask`.
3. The `NlmTask` receives Client A's command, calls `nlm_service.lock`, and gets `Granted`. It sends the reply to Client A's write task.
4. The `NlmTask` then receives Client B's command, calls `nlm_service.lock`, and gets `Denied` (because Client A holds the lock). It sends the reply to Client B's write task.

Inside the system, the following things happen and they use this module:
- **State Serialization**: By funneling all NLM calls through a single `async` task, the system ensures that the `Nlm` service implementation never needs to deal with concurrent access to its internal state. The task itself acts as a mutual exclusion mechanism.
- **Decoupling**: The connection logic (parsing bytes, managing sockets) is decoupled from the locking logic. The connection tasks only know how to construct a `NlmCommand` and how to handle a `ProcReply`. They do not know how the lock is actually granted or denied.
- **Backpressure Handling**: Although the channel is unbounded, the sequential nature of the `NlmTask` naturally applies backpressure to the locking subsystem. If the `Nlm` service is slow (e.g., due to disk I/O or network latency in a clustered lock manager), the queue in `NlmTask` will grow, but the processing remains strictly ordered.

The critical aspect of this module is the **single-threaded execution model for a multi-threaded problem**. It trades raw parallelism for correctness and simplicity in state management, relying on the asynchronous runtime to efficiently yield control while waiting for I/O within the `Nlm` service.