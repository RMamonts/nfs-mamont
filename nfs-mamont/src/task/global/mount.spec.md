<!-- SPEC_HASH: 0b300e059e3c30777aa438f3ab24ba4b4dd0556135d94b0c1bc12deafdecf359 -->
# Module Specification

Module: nfs_mamont::task::global::mount
Rust File: src/task/global/mount.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::net::SocketAddr`**: Used to identify the client peer in the `MountCommand`. This address is passed to the `Mount` service methods to potentially enforce IP-based access control or logging.
- **`std::sync::Arc`**: Used to wrap the `mount_service` (`Arc<M>`). This allows the `MountTask` to share ownership of the service implementation with the rest of the application (e.g., the main server loop) while ensuring thread-safe access.
- **`async_channel::{Receiver, Sender}`**: Used to create an unbounded asynchronous channel (`Receiver<MountCommand<B>>`, `Sender<MountCommand<B>>`). This decouples the connection handling tasks (producers) from the mount processing logic (consumer), allowing for backpressure handling and concurrent request queuing.
- **`tracing`**: Used for debug logging (`debug!`) to trace the lifecycle of mount commands (receipt, processing, result queuing).
- **`crate::allocator::Buffer`**: Used as a generic type parameter `B` for `MountCommand` and `MountTask`. This propagates the memory management constraints from the lower layers, ensuring that if the MOUNT protocol were to carry data buffers (though currently it mostly handles metadata), the types would align with the server's allocator.
- **`crate::mount::{Mount, MountRes}`**: `Mount` is the trait bound for the service `M`. It defines the interface for the actual mount logic (mnt, umnt, etc.). `MountRes` is the enum used to wrap the specific results of these procedures before sending them back to the client.
- **`crate::parser::{MountArgWrapper, MountArguments}`**: `MountArgWrapper` is the payload of `MountCommand`, containing the parsed RPC header and the specific procedure arguments (`MountArguments`). This allows the task to inspect the `xid`, credentials, and procedure type without re-parsing.
- **`crate::task::{ProcReply, ProcResult}`**: `ProcReply` is the structure sent back via the `result_tx` channel. It combines the transaction ID (`xid`) with the `ProcResult`, which wraps the `MountRes`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a **serialized execution context** for all MOUNT protocol requests received by the server. By funneling requests from multiple client connections into a single `MountTask`, the system ensures that modifications to the global mount state (e.g., adding/removing mount entries) occur sequentially, preventing race conditions without requiring complex locking within the `Mount` service implementation.
- To act as an **asynchronous bridge** between the connection layer (which reads bytes) and the service layer (which implements filesystem logic), translating `MountArgWrapper` into `ProcReply`.

Inputs:
- **`mount_service: Arc<M>`**: The shared service instance implementing the MOUNT protocol logic.
- **`command: MountCommand<B>`**: Received via the channel. Contains the parsed arguments, a sender for the reply, and the client's address.

Outputs:
- **`ProcReply<B>`**: Sent asynchronously via the `result_tx` channel contained in the incoming `MountCommand`.

Steps:
1. **Initialization (`new`)**:
   - Creates an unbounded `async_channel` for `MountCommand<B>`.
   - Instantiates `MountTask` holding the `mount_service` and the `receiver` end of the channel.
   - Returns the `MountTask` instance and the `sender` end of the channel.
2. **Spawning (`spawn`)**:
   - Takes ownership of `self`.
   - Uses `tokio::spawn` to schedule the `run` method as a background task.
   - *Note*: This panics if called outside of a Tokio runtime.
3. **Processing Loop (`run`)**:
   - Enters an infinite loop awaiting `receiver.recv().await`.
   - Destructures the `MountCommand` into `result_tx`, `client_addr`, and `args` (`MountArgWrapper`).
   - Extracts `header` (containing `xid` and `cred`) and `proc` (`MountArguments`) from `args`.
   - **Dispatch**: Matches on `proc`:
     - `MountArguments::Null`: Returns `MountRes::Null`.
     - `MountArguments::Mount(args)`: Calls `mount_service.mnt(args, client_addr, header.cred).await`. Wraps result in `MountRes::Mount`.
     - `MountArguments::Unmount(args)`: Calls `mount_service.umnt(args, client_addr).await`. Returns `MountRes::Unmount`.
     - `MountArguments::Export`: Calls `mount_service.export().await`. Wraps result in `MountRes::Export`.
     - `MountArguments::Dump`: Calls `mount_service.dump().await`. Wraps result in `MountRes::Dump`.
     - `MountArguments::UnmountAll`: Calls `mount_service.umntall(client_addr).await`. Returns `MountRes::UnmountAll`.
   - **Response Construction**: Creates a `ProcReply` struct containing the original `xid` and the `ProcResult::Mount(Box::new(mount_result))`.
   - **Reply Transmission**: Sends the `ProcReply` via `result_tx.send().await`.
   - **Error Handling**: If `result_tx.send` fails (e.g., the connection task died), the error is explicitly ignored (`let _ = ...`), and the loop continues to process the next command.

Edge Cases:
- **Channel Disconnection**: If the `receiver` returns an `Err`, the loop terminates (implicitly, as `while let Ok(...)` handles it), effectively shutting down the task if all senders are dropped.
- **Send Failure**: If the `result_tx` sender is closed (connection closed), the task does not panic; it logs the reply queueing (implicitly) and continues.
- **Service Errors**: Errors returned by `mount_service` (e.g., `mnt` returning `Err(status)`) are wrapped in the `Ok` variant of `ProcReply` (inside `MountRes`), treating them as valid protocol responses rather than task failures.

Complexity:
- **Time**: Dominated by the execution time of the `mount_service` methods. The overhead of the channel and matching is O(1).
- **Space**: O(N) where N is the number of pending `MountCommand`s in the unbounded channel.

Determinism:
- **Non-deterministic**. The order of processing depends on the order of arrival via the channel, which depends on network timing and the scheduler. However, the processing itself is deterministic relative to the input.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser`**:
  - **`MountArgWrapper`**: This module relies on the wrapper to provide the `RpcHeader` (specifically the `xid` and `cred`) and the `MountArguments` enum. The task destructures this wrapper to extract the necessary context for the service call and the transaction ID for the reply.
  - **`MountArguments`**: The task performs a pattern match on this enum to determine which specific method on the `Mount` trait to invoke.

- **From `nfs_mamont::task`**:
  - **`ProcReply`**: The task constructs this struct to send the response back. It uses the `xid` from the parsed header and wraps the `MountRes` inside a `ProcResult::Mount`.
  - **`ProcResult`**: The task uses the `Mount` variant of this enum to type-erase the specific mount result, allowing the generic write task to handle it.

- **From `nfs_mamont::mount`**:
  - **`Mount` Trait**: The task acts as a generic dispatcher for this trait. It calls the methods defined in the trait (`mnt`, `umnt`, `export`, `dump`, `umntall`) based on the parsed arguments.
  - **`MountRes`**: The task wraps the return values of the trait methods into this enum to standardize the output format before placing it in `ProcReply`.

---

## 4. Data Model

Entities:
- **`MountCommand<B: Buffer>`**: A message struct representing a MOUNT protocol request.
  - `result_tx: Sender<ProcReply<B>>`: A channel sender used to return the result to the connection task.
  - `client_addr: SocketAddr`: The network address of the client.
  - `args: MountArgWrapper`: The parsed arguments containing the RPC header and procedure payload.
- **`MountTask<M, B>`**: The actor struct processing the requests.
  - `mount_service: Arc<M>`: The shared service implementing the MOUNT logic.
  - `receiver: Receiver<MountCommand<B>>`: The channel receiver for incoming commands.

Relations:
- **`MountTask` → `MountCommand` (1:N)**: The task consumes commands from the receiver.
- **`MountCommand` → `MountArgWrapper` (Composition)**: The command owns the parsed arguments.
- **`MountCommand` → `ProcReply` (Association)**: The command contains a sender specifically for the reply corresponding to that command.

Global Invariants:
- **XID Preservation**: The `xid` in the `ProcReply` sent via `result_tx` must be identical to the `xid` in the `header` of the `MountArgWrapper` received in the command.
- **Sequential Processing**: Since `MountTask` is a single task, calls to `mount_service` are serialized. Only one mount procedure is executed at a time.

## 5. Error Model

Error Types:
- **`async_channel::SendError<ProcReply<B>>`**: Occurs if `result_tx.send` fails because the receiver has been dropped.
- **Service Errors**: Errors returned by `mount_service` methods (e.g., `mnt` returning `Err(status)`). These are not Rust `Result` errors at the task level but data variants within `MountRes`.

Error Propagation Strategy:
- **Silent Ignorance**: The result of `result_tx.send(...)` is explicitly discarded using `let _ = ...`. This means if the connection task has terminated, the mount task does not crash or log an error (other than the implicit drop of the error value).
- **Data Embedding**: Protocol-level errors (e.g., permission denied, path not found) are embedded in the `MountRes` enum and sent back to the client as a valid RPC response.

Recoverability:
- **Recoverable**: The task is designed to be resilient to connection drops. If a send fails, the task continues processing the next command in the queue.

Panics:
- **Allowed**: Yes.
- **Conditions**:
  - In `spawn`: If called outside of a Tokio runtime, `tokio::spawn` will panic.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **centralize and serialize the execution of MOUNT protocol operations** within the NFS-Mamont server. The MOUNT protocol is responsible for mapping client requests to server directories and managing the list of mounted filesystems. These operations involve shared mutable state (e.g., which client is mounted on which path). If handled concurrently by multiple connection threads, this state would require complex synchronization (locks) to prevent corruption.

The system contains a **single global worker task** (`MountTask`) that acts as the sole mutator of this state. Connection tasks, which handle the raw network I/O and parsing, do not interact with the mount service directly. Instead, they parse the incoming RPC message into a `MountArgWrapper` and send it as a `MountCommand` to the `MountTask` via an unbounded channel. The `MountTask` processes these commands one by one, invoking the appropriate methods on the `Mount` service trait (e.g., `mnt`, `umnt`). Once the service returns a result, the task wraps it in a `ProcReply` and sends it back to the specific connection task that requested it, using the one-shot channel included in the command.

A typical usage scenario of the system involves a client attempting to mount a directory.
1. The connection task reads the request, parses it, and identifies it as a MOUNT protocol request.
2. It creates a `MountCommand` containing the arguments and a channel for the reply.
3. The `MountTask` receives this command, calls `mount_service.mnt(...)`, and gets a result (e.g., a file handle or an error status).
4. The `MountTask` sends the result back to the connection task, which serializes it and sends it to the client.

Inside the system, the following things happen and they use this module:
- **State Serialization**: By design, all `Mount` trait methods are called exclusively within the `MountTask`'s `run` loop. This guarantees that the underlying service implementation does not need to handle concurrent access internally, simplifying its logic significantly.
- **Backpressure Decoupling**: The use of an unbounded channel (`async_channel`) allows the connection tasks to offload the request immediately without waiting for the mount operation to complete. The `MountTask` queues requests and processes them at its own pace.
- **Context Propagation**: The module extracts the `RpcHeader` (containing the XID and credentials) from the `MountArgWrapper` and passes the credentials to the service methods while using the XID to route the response back to the correct client context.

The critical aspect of this module is the **Actor Pattern implementation**. It transforms a potentially complex multi-threaded synchronization problem into a single-threaded sequential processing problem, which is easier to reason about and less prone to deadlocks or race conditions.