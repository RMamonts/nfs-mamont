<!-- SPEC_HASH: caff589c3ec350d0194fc3acb3d3c2ef0761b7e13a1741cf16481e4da5cffe76 -->
# Module Specification

Module: nfs_mamont::task::connection::write
Rust File: src/task/connection/write.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::net::tcp::OwnedWriteHalf`**: Used to take ownership of the write half of a TCP stream. This allows the task to exclusively handle writing data to the network socket without interfering with the read half.
- **`async_channel::Receiver<ProcReply<B>>`**: Used as the input channel for the task. It receives `ProcReply` messages (the results of RPC procedure executions) from other parts of the system (e.g., the read task or global workers) that need to be sent back to the client.
- **`crate::serializer::server::serialize_struct::Serializer`**: Used to perform the actual conversion of high-level `ProcReply` structures into the binary ONC RPC/XDR wire format. It manages buffering, Record Marking Standard (RMS) headers, and protocol-specific encoding.
- **`crate::rpc::{AuthFlavor, OpaqueAuth}`**: Used to construct the authentication verifier field required in the RPC reply header. Currently, the module hardcodes this to `AuthFlavor::None`.
- **`crate::task::ProcReply`**: The primary data type handled by the task. It wraps the result of an RPC call (success or error) along with the transaction ID (`xid`).
- **`crate::allocator::Buffer`**: A trait bound on the generic parameter `B`. It ensures that any data payloads within the `ProcReply` (such as file data from an NFS READ operation) adhere to the server's memory management interface, potentially enabling zero-copy optimizations.
- **`std::marker::PhantomData`**: Used to anchor the generic type parameter `B` to the struct at the type level without storing a value of type `B`, since `B` is only used within the `ProcReply` messages received from the channel.
- **`tracing::error`**: Used for structured logging. If serialization or writing fails, the error is logged to the diagnostic system.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Asynchronous Write Loop (`run`)

**Intent:**
To continuously consume `ProcReply` messages from a channel and serialize them onto the network socket, effectively acting as the consumer side of the server's response pipeline.

**Inputs:**
- `self`: Consumes the `WriteTask` instance, taking ownership of the `writehalf` and `result_receiver`.

**Outputs:**
- Returns `()` when the channel is closed (receiver returns `Err`).
- Side effect: Writes bytes to the TCP socket.

**Steps:**
1. Instantiate a `Serializer<B, OwnedWriteHalf>` using the owned `writehalf`.
2. Enter an infinite `while let Ok(reply) = result_receiver.recv().await` loop.
3. For each `reply` received:
 - Construct an `OpaqueAuth` verifier with `flavor: AuthFlavor::None` and an empty body.
 - Invoke `serializer.form_reply(reply, verifier).await`.
 - **If `Ok(_)`**: The reply was successfully serialized and written.
 - **If `Err(e)`**: Log the error using `tracing::error!`. The loop continues to the next message (current implementation does not terminate on error).

**Edge Cases:**
- **Channel Closure**: If the sender side of the `async_channel` is dropped, `recv()` returns `Err`, breaking the loop and ending the task (closing the connection).
- **Serialization Failure**: If `form_reply` fails (e.g., due to an I/O error or invalid data), the error is logged but the task attempts to process subsequent messages.

**Complexity:**
- Time: O(N) per message, where N is the size of the serialized reply (determined by the `Serializer`).
- Space: O(1) auxiliary space (excluding the internal buffers managed by the `Serializer`).

**Determinism:**
- Non-deterministic. The order and timing of message processing depend on when the sender produces `ProcReply` messages and the scheduling of the Tokio runtime.

### Mechanism 2: Task Spawning (`spawn`)

**Intent:**
To detach the execution of the write loop from the current context, allowing it to run concurrently on the Tokio runtime as a background task.

**Inputs:**
- `self`: Consumes the `WriteTask` instance.

**Outputs:**
- Returns `()` immediately after spawning.

**Steps:**
1. Call `tokio::spawn(async move { self.run().await })`.
2. The runtime takes ownership of the future and schedules it for execution.

**Edge Cases:**
- **Runtime Absence**: The documentation explicitly states this function panics if called outside of a Tokio runtime context.

**Complexity:**
- Time: O(1) (spawning is a lightweight operation).
- Space: O(1) stack space for the spawn closure.

**Determinism:**
- Deterministic (spawning always succeeds if runtime exists).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer::server::serialize_struct`**:
 - **`Serializer::form_reply`**: This is the core mechanism used by `WriteTask`. It abstracts away the complexity of XDR encoding, Record Marking Standard headers, and protocol-specific payload structures. The `WriteTask` relies on it to correctly translate the `ProcReply` enum into a byte stream that the client can understand.
 - **`Serializer::new`**: Used to initialize the serializer with the `OwnedWriteHalf`, effectively linking the serialization logic to the network socket.

- **From `nfs_mamont::rpc`**:
 - **`OpaqueAuth`**: The module uses this struct to satisfy the protocol requirement for a verifier in the reply header. While the module currently hardcodes the values, the struct provides the necessary type safety and layout for the RPC layer.

- **From `nfs_mamont::task`**:
 - **`ProcReply`**: This enum acts as the unified message envelope. The `WriteTask` is agnostic to the specific protocol (NFS, MOUNT, NLM) contained within; it simply passes the `ProcReply` to the serializer, which handles the dispatching internally.

---

## 4. Data Model

Entities:
- **`WriteTask<B: Buffer>`**: A struct encapsulating the state required for writing responses.
 - `writehalf`: The owned write half of the TCP stream.
 - `result_receiver`: The channel receiver for incoming `ProcReply` messages.
 - `_phantom`: A marker for the generic buffer type `B`.

Relations:
- **`WriteTask` → `OwnedWriteHalf` (1:1)**: Exclusive ownership of the socket's write capability.
- **`WriteTask` → `Receiver<ProcReply<B>>` (1:1)**: Exclusive ownership of the receive endpoint for the command channel.

Global Invariants:
- The `WriteTask` must be spawned on a Tokio runtime to function.
- The `result_receiver` must be connected to a sender that produces `ProcReply` messages matching the `xid`s expected by the client (though the task itself does not validate `xid`s, it just forwards them).

---

## 5. Error Model

Error Types:
- **`std::io::Error`**: Implicitly returned by `Serializer::form_reply` (which wraps I/O errors from the socket).

Error Propagation Strategy:
- **Logging and Suppression**: Errors returned by `serializer.form_reply` are caught in a `match` statement within the `run` loop. Instead of propagating them up or terminating the task, they are logged using `tracing::error!`, and the loop continues.

Recoverability:
- **Partial Recoverability**: If a single reply fails to serialize or send, the task logs the error and attempts to process the next reply from the channel. This implies that a transient error might not kill the connection, but a persistent socket error will likely cause all subsequent writes to fail, filling logs until the connection is closed by the peer or the read task.

Panics:
- **Allowed**: Yes.
- **Conditions**:
 - In `spawn`: If called outside of a Tokio runtime context.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **decouple the network transmission of RPC responses from the request processing logic**, enabling concurrent handling of client connections. In a high-performance NFS server, blocking on network writes or mixing read/write logic can lead to head-of-line blocking or underutilization of CPU cores. This module solves that by dedicating a specific asynchronous task (`WriteTask`) solely to consuming processed results and pushing them to the client.

This system contains a **producer-consumer pattern** where the `WriteTask` acts as the consumer. The producers (likely the read task or global protocol workers) generate `ProcReply` instances containing the results of file operations, mount requests, or locking requests. These are pushed into an `async_channel`. The `WriteTask` pulls these messages, uses the `Serializer` to convert them into the raw XDR/RPC bytes required by the NFS protocol, and writes them to the `OwnedWriteHalf` of the TCP socket.

A typical usage scenario of the system involves a client requesting a file read. The read task parses the request, dispatches it to the VFS, and receives a result containing file data. The read task wraps this in a `ProcReply` and sends it to the `result_receiver`. The `WriteTask`, waiting in its loop, receives this message. It invokes `Serializer::form_reply`, which efficiently serializes the metadata and streams the file data (potentially using zero-copy if the buffer supports it) to the socket. The `WriteTask` then immediately loops back to wait for the next reply, ensuring the network pipe is kept full without waiting for the next request to be read.

Inside the system, the following things happen and they use this module:
- **Connection Lifecycle Management**: The parent connection module splits a `TcpStream` into read and write halves. It passes the write half to `WriteTask::new` and then calls `spawn`. This effectively hands off responsibility for the outgoing data stream to this module.
- **Backpressure Handling**: Although the `WriteTask` processes messages as fast as it can, the `async_channel` provides a natural synchronization point. If the network is slow, the channel fills up, eventually causing the producers to block on `send`, effectively applying backpressure to the request processing pipeline.
- **Error Isolation**: By catching serialization errors in a loop and logging them, the module prevents a single malformed message from crashing the entire connection task immediately, though it relies on the TODO comment indicating that more sophisticated error handling (like closing the connection) might be implemented in the future.