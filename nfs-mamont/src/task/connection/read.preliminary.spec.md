<!-- SPEC_HASH: d82a16535ac2578586e9038ab080c939ed334fd26937e655629c9c2630537396 -->
# Module Specification

Module: nfs_mamont::task::connection::read
Rust File: src/task/connection/read.rs

---

## 1. Dependencies

From the provided context and dependency facts:

- **`std::io`**: Used for the `io::Result` return type of the `run` method and for constructing `io::Error` (specifically `BrokenPipe` and `Other`) when handling send failures or fatal parse errors.
- **`std::marker::PhantomData`**: Used in the `ReadTask` struct to tie the lifetime and variance of the buffer type `B` to the struct, allowing `B` to be used in generic bounds without being stored as a direct field.
- **`std::net::SocketAddr`**: Used to store the client's network address, which is passed along with MOUNT protocol requests to the `MountTask` for potential access control or logging.
- **`std::sync::Arc`**: Used to share the `Allocator` instance (`A`) between the `ReadTask` and the `RpcParser`, ensuring thread-safe access to memory management resources.
- **`tokio::net::tcp::OwnedReadHalf`**: Represents the read half of a TCP stream. The `ReadTask` takes ownership of this handle to read incoming RPC data asynchronously.
- **`tracing`**: Used for structured logging (`debug!`, `error!`) to record the dispatching of RPC procedures (including XID, program, and procedure name) and parsing errors.
- **`async_channel::Sender`**: Used for inter-task communication. The module holds senders for `MountCommand`, `NlmCommand`, `ProcReply`, and a tuple for VFS commands. These allow the `ReadTask` to offload work to global worker tasks (`MountTask`, `NlmTask`, `VfsPool`) and send replies to the `WriteTask`.
- **`crate::allocator::{Allocator, Buffer}`**: The `Allocator` trait (`A`) is used to provide memory to the `RpcParser` for buffering incoming network data. The `Buffer` trait (`B`) defines the interface for these memory chunks.
- **`crate::mount::MountRes`**: Used to construct the result for MOUNT protocol NULL procedures.
- **`crate::nlm::NlmRes`**: Used to construct the result for NLM protocol NULL procedures.
- **`crate::parser::parser_struct::RpcParser`**: The core component responsible for reading bytes from the socket and parsing them into high-level `ArgWrapper` structures. It is initialized with the socket read half and the allocator.
- **`crate::parser::{ArgWrapper, ProcArguments, ...}`**: Data structures representing parsed RPC messages. `ArgWrapper` is the main container, `ProcArguments` is the enum dispatching to NFS, MOUNT, or NLM, and specific wrappers (`NfsArgWrapper`, `MountArgWrapper`, `NlmArgWrapper`) are used to forward data to worker tasks.
- **`crate::rpc::Error`**: Used to wrap parsing errors that occur within the `RpcParser` so they can be sent back to the client as a `ProcReply`.
- **`crate::task::global::mount::MountCommand`**: The message type sent to the `MountTask`. It wraps the procedure arguments, the reply channel sender, and the client address.
- **`crate::task::global::nlm::NlmCommand`**: The message type sent to the `NlmTask`. It wraps the procedure arguments and the reply channel sender.
- **`crate::task::{ProcReply, ProcResult}`**: `ProcReply` is the generic envelope for responses sent back to the network layer. `ProcResult` is the enum distinguishing between NFS, MOUNT, and NLM results.
- **`crate::vfs::NfsRes`**: Used to construct the result for NFS protocol NULL procedures.

---

## 2. Mechanics

**Intent:**
The module implements the "read-half" of an RPC connection handler. Its purpose is to continuously read data from a TCP socket, parse it into structured RPC messages, and dispatch these messages to the appropriate service executor (VFS, Mount, or NLM tasks) or handle them directly if they are simple "NULL" procedures. It acts as a demultiplexer for incoming RPC traffic, ensuring that the network reading loop is never blocked by the execution time of the actual RPC procedures.

**Inputs:**
- **Configuration (`ReadTask::new`)**:
 - `readhalf`: The owned read half of a TCP stream.
 - `client_addr`: The socket address of the connected client.
 - `mount_sender`: Channel sender for the global MOUNT task.
 - `nlm_sender`: Channel sender for the global NLM task.
 - `result_sender`: Channel sender for the connection's write task (used for replies).
 - `allocator`: A reference-counted memory allocator.
 - `pool_sender`: Channel sender for the global VFS pool task.

**Outputs:**
- **Task Instance**: A `ReadTask` ready to be spawned.
- **Side Effects**:
 - Sending `MountCommand` messages to the MOUNT task.
 - Sending `NlmCommand` messages to the NLM task.
 - Sending `(NfsArgWrapper, Sender)` tuples to the VFS pool.
 - Sending `ProcReply` messages directly to the `WriteTask` (for NULL procedures or errors).
 - Logging debug and error messages.

**Steps:**

1. **Initialization (`ReadTask::new`)**:
 - Constructs the `ReadTask` struct, storing all provided channels and handles.

2. **Spawning (`ReadTask::spawn`)**:
 - Consumes the `ReadTask` and uses `tokio::spawn` to run the `run` method asynchronously.
 - **Panics** if called outside of a Tokio runtime context.

3. **Execution Loop (`ReadTask::run`)**:
 - Instantiates `RpcParser` with the `readhalf` and `allocator`.
 - Enters an infinite loop calling `parser.next_message().await`.
 - **Dispatch Logic**:
 - **NFS NULL**: If `proc` is `Nfs3(Null)`, constructs a `ProcReply` with `NfsRes::Null` and sends it to `result_sender`.
 - **NLM NULL**: If `proc` is `Nlm4(Null)`, constructs a `ProcReply` with `NlmRes::Null` and sends it to `result_sender`.
 - **NFS Non-NULL**: If `proc` is `Nfs3(Non-Null)`, constructs a `NfsArgWrapper` and sends it along with a clone of `result_sender` to `pool_sender`.
 - **MOUNT NULL**: If `proc` is `Mount(Null)`, constructs a `ProcReply` with `MountRes::Null` and sends it to `result_sender`.
 - **MOUNT Non-NULL**: If `proc` is `Mount(Non-Null)`, constructs a `MountCommand` containing the arguments, `client_addr`, and `result_sender`, then sends it to `mount_sender`.
 - **NLM Non-NULL**: If `proc` is `Nlm4(Non-Null)`, constructs an `NlmCommand` containing the arguments and `result_sender`, then sends it to `nlm_sender`.
 - **Parse Error (with XID)**: If parsing fails but an XID is available, constructs a `ProcReply` containing the error and sends it to `result_sender`.
 - **Parse Error (no XID)**: If parsing fails and no XID is available (meaning the stream is corrupt beyond recognition), logs an error and returns `Err(io::Error)`, terminating the loop.
 - **Send Failure Handling**: If any send operation to `result_sender` fails, the `send_broken_pipe` helper is called. This helper attempts to send a `BrokenPipe` error reply and then returns an `io::Error`, terminating the loop.

**Edge Cases:**
- **Broken Pipe**: If the `result_sender` (connection to the write task) is closed, the task attempts to notify the peer of the failure via `send_broken_pipe` and then terminates.
- **Fatal Parse Error**: If the parser cannot determine the XID, the task terminates immediately as it cannot correlate a response with a request.
- **NULL Procedure Optimization**: NULL procedures are handled entirely within the read loop without context switching to worker tasks, minimizing latency for these common "ping" operations.

**Complexity:**
- **Time**: Per-message latency is dominated by `parser.next_message()` (network I/O and parsing). Dispatch logic is O(1).
- **Space**: O(1) for the task state. The `RpcParser` holds internal buffers managed by the `Allocator`.

**Determinism:**
- **Non-deterministic** regarding the order of incoming messages and the timing of channel operations. However, the processing logic for a given message is deterministic.

---

## 3. Dependency Mechanics

- **`RpcParser` (from `nfs_mamont::parser::parser_struct`)**:
 - **Stream Processing**: The `ReadTask` relies on `RpcParser` to handle the low-level details of reading the RPC record marking standard (fragment headers) and deserializing the payload into Rust structs. The parser abstracts away the need for manual buffer management in the `ReadTask`.
 - **Allocation**: The parser uses the provided `Arc<A>` to allocate memory for incoming data, ensuring that the `ReadTask` does not perform dynamic heap allocations directly.

- **`async_channel::Sender`**:
 - **Decoupling**: The module uses unbounded channels to send commands to global tasks. This allows the `ReadTask` to immediately queue a request for processing and return to reading the socket, preventing a slow backend operation from blocking the network reception.
 - **Reply Routing**: By cloning `result_sender` and passing it inside command messages (e.g., `MountCommand`), the module enables the worker tasks to send responses directly back to the specific connection's write task, bypassing the `ReadTask` entirely for the return path.

- **`MountTask` / `NlmTask` / `VfsPool`**:
 - **Protocol Dispatch**: The `ReadTask` acts as a protocol router. It inspects the `ProcArguments` enum to determine which global task is responsible for the request. This design allows a single TCP connection to multiplex requests for different logical services (NFS, MOUNT, NLM) over the same stream.

---

## 4. Data Model

**Entities:**
- **`ReadTask<A, B>`**: The actor struct encapsulating the read-half of a connection. It holds the necessary state to read, parse, and dispatch messages.

**Relations:**
- **`ReadTask` → `OwnedReadHalf` (1:1)**: Exclusive ownership of the socket's read capability.
- **`ReadTask` → `Sender<...>` (1:N)**: Holds references to multiple channels (Mount, NLM, VFS, Write) to facilitate dispatch and reply.

**Global Invariants:**
- **XID Preservation**: The `xid` (Transaction ID) extracted from the RPC header must be preserved in the `ProcReply` sent to `result_sender` to ensure the client can match the response to the request.
- **Client Address Consistency**: The `client_addr` passed to `MountCommand` must be the address of the peer associated with the `readhalf`.

---

## 5. Error Model

**Error Types:**
- **`io::Error`**: Returned by the `run` method. Typically `BrokenPipe` (if the reply channel is closed) or `Other` (if parsing fails catastrophically).
- **`rpc::Error`**: Wrapped inside `ProcReply` and sent to the client when parsing fails but an XID is available.

**Error Propagation Strategy:**
- **Recoverable Parse Errors**: If `RpcParser` returns an error with an XID, the error is wrapped in a `ProcReply` and sent to the client. The loop continues.
- **Fatal Parse Errors**: If `RpcParser` returns an error without an XID, the `run` method returns `Err(io::Error)`, terminating the task.
- **Channel Send Errors**: If sending to `result_sender` fails, `send_broken_pipe` is invoked. It attempts to send a final `BrokenPipe` error and then returns `Err(io::Error)`, terminating the task.

**Recoverability:**
- **Partial**: The task can recover from parse errors that have an XID. It cannot recover from a closed `result_sender` or a completely corrupt stream (no XID), as these imply the connection is dead or the protocol state is unrecoverable.

**Panics:**
- **Allowed**: Yes.
- **Conditions**:
 - In `spawn`: If `tokio::spawn` is called outside of a Tokio runtime.
 - Potential panics within `RpcParser` or channel operations will propagate and crash the task.

---

## 6. Traits

The module does not define any traits. It uses the following traits as bounds:
- **`A: Allocator + Send + Sync + 'static`**: Ensures the allocator can be shared across threads and used in an async context.
- **`B: Buffer + 'static`**: Ensures the buffer type meets the interface requirements and has a static lifetime.

---

## 7. Overview

This module is used in order to **implement the ingress logic for RPC connections**, serving as the bridge between the raw network byte stream and the high-level service executors. It solves the problem of protocol multiplexing and I/O decoupling by ensuring that reading data from the network is a continuous, non-blocking operation, regardless of how long the actual processing of a request might take.

This system contains a **dispatcher pattern** where the `ReadTask` is responsible for "front-door" duties: authentication context extraction (via the parser), protocol identification, and request routing. It optimizes for the common case of NULL procedures by handling them synchronously within the loop, avoiding the overhead of inter-task communication for these no-op requests.

A typical usage scenario of the system involves a client sending a mix of NFS file operations and MOUNT requests over a single TCP connection. The `ReadTask` reads a message, identifies it as a MOUNT request, and wraps it in a `MountCommand` containing the client's IP address. This command is sent to the `MountTask`. Immediately after, the `ReadTask` reads an NFS READ request. It wraps this in a tuple with a reply channel and sends it to the `VfsPool`. The `ReadTask` then immediately goes back to reading the socket, while the other tasks handle the logic asynchronously.

Inside the system, the following things happen and they use this module:
1. **Protocol Demultiplexing**: The system relies on `ReadTask` to inspect the program number in the RPC header and direct the traffic to the correct subsystem (VFS for NFS, `MountTask` for MOUNT, `NlmTask` for NLM).
2. **Connection Lifecycle Management**: The module manages the lifetime of the read-half of the socket. If the connection breaks (indicated by send failures or fatal parse errors), the task terminates, effectively signaling the end of the connection handling for that specific client.
3. **Context Propagation**: The module ensures that connection-specific context (like the `client_addr` and the specific `result_sender` channel) is attached to the commands sent to global tasks. This allows stateless global tasks to generate responses that are routed correctly back to the specific client that made the request.

The critical aspect of this module is the **separation of concerns**: it isolates the mechanics of network reception and protocol parsing from the business logic of file serving, locking, and mounting.