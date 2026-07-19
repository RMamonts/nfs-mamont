<!-- SPEC_HASH: d82a16535ac2578586e9038ab080c939ed334fd26937e655629c9c2630537396 -->
# Module Specification

Module: nfs_mamont::task::connection::read
Rust File: src/task/connection/read.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**: Used for the `io::Result` type and `io::Error` construction. Specifically, it is used to signal termination of the read loop when a fatal parsing error occurs (e.g., missing XID) or when a channel send fails.
- **`std::marker::PhantomData`**: Used to make the `ReadTask` struct generic over the buffer type `B` without actually storing a value of type `B`. This ensures the struct respects variance and lifetime rules associated with the `Buffer` generic parameter.
- **`std::net::SocketAddr`**: Used to store the client's network address (`client_addr`). This is passed along with MOUNT protocol requests to the global `MountTask`, as the MOUNT protocol may require the client's IP address for access control or logging.
- **`std::sync::Arc`**: Used to wrap the `Allocator` instance. This allows the `ReadTask` to share ownership of the memory allocator with the `RpcParser` and potentially other tasks, ensuring thread-safe access to the memory pool.
- **`tokio::net::tcp::OwnedReadHalf`**: Represents the read half of a TCP stream. The `ReadTask` takes ownership of this handle to asynchronously read incoming RPC data from the network.
- **`tracing`**: Used for structured logging (`debug!`, `error!`). It logs the dispatching of RPC commands (including XID, program, and procedure) and parsing errors, providing observability into the request flow.
- **`async_channel::Sender`**: Used for sending messages to other tasks.
  - `Sender<MountCommand<B>>`: Sends parsed MOUNT protocol requests to the global `MountTask`.
  - `Sender<NlmCommand<B>>`: Sends parsed NLM protocol requests to the global `NlmTask`.
  - `Sender<ProcReply<B>>`: Sends responses (either successful results or errors) directly to the connection's write task. This is used for "NULL" procedures (handled locally) and error replies.
  - `Sender<(NfsArgWrapper<B>, Sender<ProcReply<B>>)>`: Sends parsed NFS protocol requests to the VFS worker pool. The tuple includes the arguments and a channel sender for the VFS worker to return the result.
- **`crate::allocator::{Allocator, Buffer}`**: Defines the memory management interface. `Allocator` is used to provide memory to the `RpcParser` for reading data, and `Buffer` is the generic type representing the allocated memory blocks.
- **`crate::mount::MountRes`**: Used to construct the result payload for MOUNT protocol "NULL" procedures, which are handled directly by this task without forwarding to the global `MountTask`.
- **`crate::nlm::NlmRes`**: Used to construct the result payload for NLM protocol "NULL" procedures.
- **`crate::parser::parser_struct::RpcParser`**: The core component responsible for reading bytes from the `OwnedReadHalf` and parsing them into high-level `ArgWrapper` structures. It uses the provided `Allocator` to manage buffer memory.
- **`crate::parser::{ArgWrapper, ErrorWrapper, MountArgWrapper, NfsArgWrapper, NlmArgWrapper, ProcArguments, ...}`**: Defines the data structures representing parsed RPC messages. `ArgWrapper` is the main enum discriminating between protocols (NFS, MOUNT, NLM). `ErrorWrapper` is used to handle parsing failures.
- **`crate::rpc::Error`**: Represents RPC-level errors (e.g., authentication failures, garbage args). These are wrapped in `ProcReply` and sent back to the client if parsing fails but an XID is available.
- **`crate::task::global::mount::MountCommand`**: The message structure sent to the `MountTask`. It wraps the parsed arguments, the client address, and a sender for the reply.
- **`crate::task::global::nlm::NlmCommand`**: The message structure sent to the `NlmTask`. It wraps the parsed arguments and a sender for the reply.
- **`crate::task::{ProcReply, ProcResult}`**: Defines the structure of the response message sent back to the write task. `ProcReply` contains the XID and the result (either `ProcResult` or `Error`).
- **`crate::vfs::NfsRes`**: Used to construct the result payload for NFS protocol "NULL" procedures.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement the **ingress and dispatch logic** for a single client connection. The module reads raw bytes from the network, parses them into structured RPC commands, and routes them to the appropriate service handler (Global Mount Task, Global NLM Task, VFS Pool, or local handling for NULL procedures).
- To **optimize latency for NULL procedures** by handling them immediately within the read task, avoiding the overhead of channel communication and context switching to global tasks.
- To **manage the lifecycle of the read half** of a TCP connection, ensuring graceful termination if parsing fails or communication channels break.

Inputs:
- **`readhalf`**: The asynchronous read handle of the TCP connection.
- **`client_addr`**: The socket address of the connected client.
- **`mount_sender`**: Channel sender for the global MOUNT task.
- **`nlm_sender`**: Channel sender for the global NLM task.
- **`result_sender`**: Channel sender for the connection's write task.
- **`allocator`**: Shared reference to the memory pool.
- **`pool_sender`**: Channel sender for the VFS worker pool.

Outputs:
- **Messages to Global Tasks**: `MountCommand` sent to `mount_sender`, `NlmCommand` sent to `nlm_sender`, and `(NfsArgWrapper, Sender)` sent to `pool_sender`.
- **Messages to Write Task**: `ProcReply` sent to `result_sender` (for NULL procedures and errors).
- **`io::Result<()>`**: Returned by the `run` method, indicating success or failure of the read loop.

Steps:
1. **Initialization (`spawn`)**:
 - The `spawn` method takes ownership of `self` and uses `tokio::spawn` to schedule the `run` method as an asynchronous background task.
2. **Parser Creation (`run`)**:
 - Instantiates `RpcParser` with the `readhalf` and `allocator`.
3. **Message Loop (`run`)**:
 - Enters an infinite loop calling `parser.next_message().await`.
4. **Dispatch Logic**:
 - **NFS NULL**: If the parsed message is NFSv3 NULL, constructs a `ProcReply` with `NfsRes::Null` and sends it to `result_sender`.
 - **NLM NULL**: If the parsed message is NLMv4 NULL, constructs a `ProcReply` with `NlmRes::Null` and sends it to `result_sender`.
 - **NFS Non-NULL**: Constructs a tuple `(NfsArgWrapper, result_sender.clone())` and sends it to `pool_sender`.
 - **MOUNT NULL**: If the parsed message is MOUNT NULL, constructs a `ProcReply` with `MountRes::Null` and sends it to `result_sender`.
 - **MOUNT Non-NULL**: Constructs a `MountCommand` containing the arguments, client address, and `result_sender.clone()`, then sends it to `mount_sender`.
 - **NLM Non-NULL**: Constructs an `NlmCommand` containing the arguments and `result_sender.clone()`, then sends it to `nlm_sender`.
5. **Error Handling**:
 - **Error with XID**: If parsing returns an `ErrorWrapper` with an `xid`, constructs a `ProcReply` containing the error and sends it to `result_sender`.
 - **Error without XID**: If parsing returns an `ErrorWrapper` without an `xid`, logs the error and returns an `io::Error`, terminating the loop.
6. **Send Failure Handling**:
 - If any send operation to a channel fails (returns `Err`), the `send_broken_pipe` helper is called. This helper sends a `ProcReply` with a `BrokenPipe` error to `result_sender` (to notify the write task) and then returns an `io::Error`, terminating the loop.

Edge Cases:
- **Broken Pipe**: If the `result_sender` (connection to the write task) is closed, the task attempts to notify the write task one last time via `send_broken_pipe` before exiting.
- **Unrecoverable Parse Error**: If the parser cannot determine the XID, the task cannot send a meaningful reply to the client, so it terminates the connection immediately.

Complexity:
- **Time**: O(1) for dispatch logic (pattern matching and channel sends). The overall time is dominated by `parser.next_message()`, which depends on network latency and message size.
- **Space**: O(1) additional space per message (stack allocation of wrappers). The parser manages buffer memory via the `Allocator`.

Determinism:
- **Non-deterministic**. The flow depends on the order and content of messages arriving from the network and the availability of the asynchronous runtime.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::parser_struct`**:
 - **`RpcParser::next_message`**: This is the primary driver of the loop. The module relies on this method to block asynchronously until a full RPC message is received and parsed, returning either an `ArgWrapper` (success) or `ErrorWrapper` (failure).

- **From `nfs_mamont::parser`**:
 - **`ArgWrapper` and `ProcArguments`**: The module uses these to perform protocol-level dispatching. It matches on `ProcArguments` to determine if the message is NFS, MOUNT, or NLM, and further matches on the inner arguments to distinguish NULL procedures from actual operations.
 - **`ErrorWrapper`**: Used to handle parsing failures. The presence or absence of the `xid` field dictates whether the task can send a reply or must terminate.

- **From `nfs_mamont::task::global::mount`**:
 - **`MountCommand`**: The module constructs this struct to encapsulate the MOUNT request. It relies on the structure of `MountCommand` (specifically the `result_tx` field) to route the response back to the correct connection context.

- **From `nfs_mamont::task::global::nlm`**:
 - **`NlmCommand`**: Similar to `MountCommand`, this struct is used to encapsulate NLM requests for the global NLM task.

- **From `nfs_mamont::task`**:
 - **`ProcReply`**: The module constructs this struct for all responses it generates directly (NULL procedures, errors). It ensures the `xid` from the request header is preserved in the reply.

---

## 4. Data Model

Entities:
- **`ReadTask<A, B>`**: The actor struct managing the read side of a connection.
 - `readhalf`: `OwnedReadHalf` - The network stream.
 - `client_addr`: `SocketAddr` - Client identifier.
 - `mount_sender`: `Sender<MountCommand<B>>` - Output channel for MOUNT.
 - `nlm_sender`: `Sender<NlmCommand<B>>` - Output channel for NLM.
 - `result_sender`: `Sender<ProcReply<B>>` - Output channel for replies.
 - `allocator`: `Arc<A>` - Memory manager.
 - `pool_sender`: `Sender<(NfsArgWrapper<B>, Sender<ProcReply<B>>)>` - Output channel for NFS.
 - `_phantom`: `PhantomData<B>` - Marker for generic type.

Relations:
- **`ReadTask` → `RpcParser` (Composition)**: The task creates and owns the parser for the duration of the connection.
- **`ReadTask` → Channels (Association)**: The task holds senders to communicate with the rest of the system (Mount, NLM, VFS, Write task).

Global Invariants:
- **XID Preservation**: For any `ProcReply` sent via `result_sender`, the `xid` must match the `xid` of the corresponding request header found in the `ArgWrapper` or `ErrorWrapper`.
- **Channel Consistency**: The `result_sender` passed to global tasks (via `MountCommand` or the tuple for VFS) must be a clone of the `ReadTask`'s own `result_sender`, ensuring replies route back to the correct write task.

## 5. Error Model

Error Types:
- **`std::io::Error`**: Returned by the `run` method when the connection must be terminated (e.g., unrecoverable parse error, broken pipe).
- **`crate::rpc::Error`**: Embedded in `ProcReply` and sent to the client when a parse error occurs that is recoverable enough to send a rejection (i.e., an XID is available).

Error Propagation Strategy:
- **Channel Send Errors**: If sending to `result_sender` fails, `send_broken_pipe` is invoked. This function attempts to send a final `BrokenPipe` error to `result_sender` and then returns an `io::Error` to the caller, propagating the failure up to terminate the task.
- **Parse Errors**:
 - If `xid` is present: The error is wrapped in `ProcReply` and sent to `result_sender`. The loop continues.
 - If `xid` is absent: An `io::Error` is returned immediately, terminating the loop.

Recoverability:
- **Recoverable**: Parse errors with an XID allow the server to notify the client of the failure and continue processing subsequent messages on the same connection.
- **Terminal**: Parse errors without an XID, or broken pipe errors in the `result_sender` channel, result in the termination of the `ReadTask`.

Panics:
- **Allowed**: Yes.
- **Conditions**:
 - In `spawn`: If called outside of a Tokio runtime.
 - Implicitly: If any of the `expect` or `unwrap` logic within the dependencies (like `RpcParser`) panics, it will propagate here.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **implement the read-side ingress logic for individual client connections in the NFS-Mamont server**. It acts as the first stage of request processing, responsible for converting raw network bytes into structured commands and dispatching them to the appropriate execution units.

The system contains a **multi-stage request processing pipeline**. The `ReadTask` sits at the entry of this pipeline. It is instantiated for every accepted TCP connection. Its primary role is to read data, parse it using the `RpcParser`, and act as a router.
- **Routing Logic**: It inspects the parsed `ArgWrapper` to determine the protocol (NFS, MOUNT, NLM).
- **Optimization**: It recognizes "NULL" procedures (which are essentially pings) for all protocols. Instead of forwarding these to the global workers, it handles them immediately by constructing a success reply and sending it to the write task. This minimizes latency for simple health checks.
- **Delegation**: For actual operations (Non-NULL), it forwards the request to specialized global tasks. MOUNT and NLM requests go to single-threaded global actors (`MountTask`, `NlmTask`) to serialize state. NFS requests go to a `pool_sender` (assumed to lead to a `VfsPool`) to be processed by a worker thread, allowing for parallelism.

A typical usage scenario of the system involves a client sending an NFS READ request.
1. The `ReadTask` reads the bytes from the socket.
2. The `RpcParser` parses the bytes into an `ArgWrapper` containing `NfsArguments::Read`.
3. The `ReadTask` matches this, creates a tuple `(NfsArgWrapper, result_sender.clone())`, and sends it via `pool_sender`.
4. The `ReadTask` immediately loops back to read the next message, while the VFS worker handles the READ operation asynchronously.

Inside the system, the following things happen and they use this module:
- **Backpressure Handling**: By relying on bounded or unbounded channels (depending on the channel implementation used in `ServerContext`), the `ReadTask` effectively decouples the network read speed from the processing speed. If the channels fill up, the `send` operations will await, naturally slowing down the read loop.
- **Error Containment**: The module ensures that parsing errors are either reported back to the client (if possible) or result in the connection being dropped, preventing malformed data from propagating deeper into the system.
- **Context Propagation**: It attaches the `client_addr` to MOUNT commands, ensuring the global `MountTask` has the necessary context to enforce IP-based access rules.

The critical aspect of this module is the **separation of concerns**: it handles *transport* and *syntax* (reading and parsing) while delegating *semantics* (filesystem operations, locking, mounting) to other specialized tasks. This allows the read loop to remain simple and responsive.