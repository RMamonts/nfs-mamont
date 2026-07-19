<!-- SPEC_HASH: 0b300e059e3c30777aa438f3ab24ba4b4dd0556135d94b0c1bc12deafdecf359 -->
# Module Specification

Module: nfs_mamont::task::global::mount
Rust File: src/task/global/mount.rs

---

## 1. Dependencies

From the provided context and dependency facts:

- **`std::net::SocketAddr`**: Used to identify the client endpoint initiating the mount request, which is passed to the underlying mount service for access control or logging.
- **`std::sync::Arc`**: Used to share the `Mount` service instance (`M`) between the task creator and the spawned task, allowing thread-safe reference counting.
- **`async_channel::{Receiver, Sender}`**: Provides the unbounded MPSC channel used to decouple the `MountTask` from connection read tasks. `Sender<MountCommand<B>>` is returned to the producer, while `Receiver` is consumed by the task.
- **`tracing`**: Used for structured logging (`debug!`) to track the lifecycle of mount commands, arguments (like directory paths), and operation results (OK/ERR).
- **`crate::allocator::Buffer`**: A trait bound `B: Buffer` for the `MountCommand` and `MountTask`. It ensures that any buffers involved in the command structure (though not directly manipulated by this task) adhere to the system's memory management interface.
- **`crate::mount::{Mount, MountRes}`**: The core business logic dependency. `Mount` is the trait defining the mount service operations (`mnt`, `umnt`, `export`, etc.), and `MountRes` is the enum wrapping the results of these operations.
- **`crate::parser::{MountArgWrapper, MountArguments}`**: Input data structures. `MountArgWrapper` wraps the RPC header and the specific procedure arguments (`MountArguments` enum) parsed from the network stream.
- **`crate::task::{ProcReply, ProcResult}`**: Output data structures. `ProcReply` wraps the result to be sent back to the network layer, and `ProcResult` is the tagged union containing the specific `MountRes`.

---

## 2. Mechanics

**Intent:**
The module implements an asynchronous worker task (`MountTask`) responsible for processing MOUNT protocol requests. Its purpose is to offload the execution of mount operations from the network I/O loop, serializing access to the `Mount` service, and bridging the gap between parsed RPC arguments and the generic task reply mechanism.

**Inputs:**
- **Configuration (`MountTask::new`)**:
  - `mount_service`: An `Arc<M>` where `M` implements the `Mount` trait.
- **Runtime Input (`MountCommand`)**:
  - `result_tx`: A channel sender to return the result.
  - `client_addr`: The socket address of the client.
  - `args`: A `MountArgWrapper` containing the RPC header and the specific `MountArguments`.

**Outputs:**
- **Task Instance**: A `MountTask` instance ready to be spawned.
- **Command Sender**: A `Sender<MountCommand<B>>` used to feed requests to the task.
- **Side Effect**: Asynchronous sending of `ProcReply<B>` messages via the `result_tx` channel contained in incoming commands.

**Steps:**

1. **Initialization (`MountTask::new`)**:
   - Creates an unbounded `async_channel` pair (`sender`, `receiver`).
   - Instantiates `MountTask` holding the `mount_service` and the `receiver`.
   - Returns the task instance and the `sender` to the caller.

2. **Spawning (`MountTask::spawn`)**:
   - Consumes the `MountTask` instance.
   - Uses `tokio::spawn` to schedule the `run` method as an asynchronous background task.
   - **Panics** if called outside of a Tokio runtime context.

3. **Execution Loop (`MountTask::run`)**:
   - Enters an infinite `while let Ok(command) = receiver.recv().await` loop.
   - Destructures the `MountCommand` into `result_tx`, `client_addr`, and `args`.
   - Destructures `args` into `header` (containing `xid` and `cred`) and `proc` (the `MountArguments` enum).
   - **Dispatch**: Matches on `*proc`:
     - `Null`: Returns `MountRes::Null`.
     - `Mount(args)`: Calls `mount_service.mnt(args, client_addr, header.cred).await`. Wraps the `Result` in `MountRes::Mount`.
     - `Unmount(args)`: Calls `mount_service.umnt(args, client_addr).await`. Returns `MountRes::Unmount`.
     - `Export`: Calls `mount_service.export().await`. Wraps the result in `MountRes::Export`.
     - `Dump`: Calls `mount_service.dump().await`. Wraps the result in `MountRes::Dump`.
     - `UnmountAll`: Calls `mount_service.umntall(client_addr).await`. Returns `MountRes::UnmountAll`.
   - **Response Construction**: Creates a `ProcReply` struct with the original `xid` and the `ProcResult::Mount(Box::new(mount_result))`. Note that the `proc_result` field is always `Ok(...)`, even if the underlying mount operation failed; the specific error status is contained within `MountRes`.
   - **Reply Transmission**: Attempts to send the `ProcReply` via `result_tx.send(...).await`. Errors during send are explicitly ignored (`let _`), ensuring the loop continues even if the receiver (WriteTask) has dropped the channel.

**Edge Cases:**
- **Channel Disconnection**: If all senders are dropped, `receiver.recv().await` returns `Err`, causing the `while` loop to terminate and the task to finish gracefully.
- **Send Failure**: If `result_tx` is closed (e.g., the client disconnected or the write task crashed), `send` fails. The task ignores this error and continues processing the next command, preventing a single failure from stopping the entire mount service.
- **Service Errors**: Errors returned by `mount_service` methods are wrapped in the `MountRes` enum (e.g., `MountRes::Mount(Err(status))`) rather than causing the task to panic or return a `Result` from the run loop.

**Complexity:**
- **Time**: O(1) for channel operations. The execution time depends entirely on the latency of the `mount_service` methods (which may involve I/O).
- **Space**: O(1) for the task state. The channel is unbounded, so memory usage grows with the backlog of `MountCommand`s if the consumer is slower than producers.

**Determinism:**
- **Non-deterministic** regarding execution order and timing due to asynchronous scheduling (`tokio::spawn`) and the unbounded channel nature (FIFO order is preserved, but timing varies). However, the logic processing each command is deterministic.

---

## 3. Dependency Mechanics

- **`async_channel` (Unbounded Channel)**:
  - **Decoupling**: The module relies on the unbounded channel to act as an asynchronous buffer. This allows the connection read tasks (producers) to submit mount requests without blocking, even if the `MountTask` (consumer) is currently busy with a long-running operation.
  - **Backpressure**: The use of an *unbounded* channel implies a lack of backpressure at this specific stage. If the `MountTask` cannot keep up, the channel queue grows indefinitely in memory until the system runs out of RAM.

- **`crate::mount::Mount`**:
  - **Abstraction**: The module treats the `Mount` trait as a black-box service provider. It does not implement the logic for mounting/unmounting but delegates it. This allows the `MountTask` to focus on protocol handling (dispatching based on `MountArguments`) and lifecycle management.
  - **Async Interface**: The `MountTask` assumes the methods on `M` are `async` and must be awaited, fitting naturally into the `tokio` runtime.

- **`crate::parser::MountArguments`**:
  - **Pattern Matching**: The module uses the structure of the `MountArguments` enum to drive control flow. The specific variants (`Mount`, `Unmount`, etc.) directly map to the methods available on the `Mount` trait.

- **`crate::task::ProcReply`**:
  - **Generic Wrapper**: The module constructs a `ProcReply` to normalize the output. It places the specific `MountRes` inside the generic `ProcResult::Mount` variant, allowing the downstream `WriteTask` to handle it generically without knowing it was a MOUNT protocol specific request.

---

## 4. Data Model

**Entities:**
- **`MountTask<M, B>`**: The actor struct. Holds the state required to process requests: a link to the service and the input queue.
- **`MountCommand<B>`**: The message packet. Transient data structure transferring ownership of request data (args) and a handle for the response (channel sender) between tasks.

**Relations:**
- **`MountTask` → `Arc<M>` (1:1)**: The task holds a reference-counted pointer to the mount service.
- **`MountTask` → `Receiver<MountCommand<B>>` (1:1)**: The task owns the receiving end of the command queue.
- **`MountCommand` → `Sender<ProcReply<B>>` (1:1)**: Each command carries a dedicated sender to route the response back to the specific context that requested it (usually a specific client connection's write task).

**Global Invariants:**
- **Sequential Processing**: Since `run` processes commands one by one in a loop, calls to the `mount_service` are serialized. Only one mount operation is executed at a time per `MountTask` instance.
- **XID Preservation**: The `xid` (Transaction ID) from the request header is copied into the `ProcReply` to ensure the client can match the response to the request.

---

## 5. Error Model

**Error Types:**
- No specific error types are defined within this module.

**Error Propagation Strategy:**
- **Service Errors**: Errors returned by the `mount_service` (e.g., `mnt` returning `Err(status)`) are captured within the `MountRes` enum variants. They are not treated as exceptions in the control flow but as data to be sent back to the client.
- **Task Errors**: Errors during `result_tx.send` are caught and discarded using `let _`. This prevents the task from crashing if the connection dies before the response is sent.

**Recoverability:**
- **Service Failures**: Recoverable. The task continues to the next command regardless of whether the previous mount operation succeeded or failed.
- **Channel Send Failures**: Recoverable. The task logs the reply (implicitly, by attempting to send) and continues, assuming the connection is dead.

**Panics:**
- **Allowed**: Yes.
- **Conditions**:
  - In `spawn`: If `tokio::spawn` is called outside of a Tokio runtime context.
  - Potential panics within `mount_service` calls are not caught here and will propagate, crashing the task.

---

## 6. Traits

The module does not define any traits. It uses the following traits as bounds:
- **`M: Mount + Send + Sync + 'static`**: Ensures the mount service can be shared across threads and sent to the async task.
- **`B: Buffer + 'static`**: Ensures the buffer type meets the allocator interface requirements.

---

## 7. Overview

This module is used in order to **provide a dedicated asynchronous execution context for the NFS MOUNT protocol**, decoupling the protocol logic from the network transport layer. It solves the problem of handling potentially blocking or complex filesystem operations (mounting, unmounting, exporting lists) without stalling the network read/write loops that handle high-throughput RPC traffic.

This system contains a **worker task pattern** where a central `MountTask` consumes commands from an unbounded channel. It acts as a bridge between the generic RPC parsing layer (which produces `MountArgWrapper`) and the specific filesystem abstraction layer (the `Mount` trait). It ensures that MOUNT protocol requests are processed sequentially, which simplifies the concurrency requirements of the underlying `Mount` service implementation.

A typical usage scenario of the system involves a client connecting to the NFS server and sending a MOUNT request. The connection read task parses the bytes into a `MountArgWrapper`, creates a `MountCommand` containing a channel sender for the reply, and sends this command to the `MountTask`'s channel. The `MountTask` wakes up, extracts the arguments, calls `mount_service.mnt(...)`, and awaits the result. Once complete, it packages the result (success or failure) into a `ProcReply` and sends it back via the provided channel. The write task then picks up this reply and serializes it back to the client.

Inside the system, the following things happen and they use this module:
1.  **Protocol Dispatch**: The system relies on `MountTask` to interpret the `MountArguments` enum and invoke the correct method on the `Mount` service (e.g., distinguishing between `MNT` and `UMNT`).
2.  **State Isolation**: By using an `Arc<M>`, the module allows the mount service state (e.g., the list of currently mounted directories) to be safely shared and accessed by this single worker, preventing race conditions that might occur if multiple connection tasks tried to modify the mount table simultaneously.
3.  **Lifecycle Management**: The module handles the "fire-and-forget" nature of RPC requests. It ensures that even if the network connection drops before the operation finishes, the operation runs to completion (or cancellation by the runtime), but the result sending failure is handled gracefully without crashing the server component.

The critical aspect of this module is the **translation of the parsed RPC message into a service call and back into a generic RPC reply**, effectively isolating the protocol-specific logic from the transport mechanics.