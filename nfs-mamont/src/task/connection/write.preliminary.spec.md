<!-- SPEC_HASH: caff589c3ec350d0194fc3acb3d3c2ef0761b7e13a1741cf16481e4da5cffe76 -->
# Module Specification

Module: nfs_mamont::task::connection::write
Rust File: src/task/connection/write.rs

---

## 1. Dependencies

From the code analysis, the following external crates and internal modules are used:

- **`tokio::net::tcp::OwnedWriteHalf`**: Used to take exclusive ownership of the write portion of a TCP stream. This allows the task to write data to the network connection without needing a mutable reference to the full `TcpStream`, facilitating the split of read and write operations into separate tasks.
- **`async_channel::Receiver`**: Used to receive `ProcReply<B>` messages from another asynchronous task (likely the request processor). This channel acts as the MPSC (multi-producer, single-consumer) boundary decoupling request processing from network writing.
- **`tracing`**: Used for structured logging. Specifically, `error!` is employed to log failures during the serialization or transmission of replies.
- **`std::marker::PhantomData`**: Used to anchor the generic buffer type `B` within the `WriteTask` struct, ensuring the struct is aware of the lifetime and type parameters of `B` even if no direct field of type `B` is stored.
- **`crate::rpc::{AuthFlavor, OpaqueAuth}`**: Used to construct the authentication verifier field required by the RPC protocol. The module currently hardcodes `AuthFlavor::None`.
- **`crate::serializer::server::serialize_struct::Serializer`**: Used to perform the actual encoding of the `ProcReply` structure into the binary ONC RPC wire format. The `WriteTask` delegates the complex logic of formatting the message (headers, body, verifier) to this serializer.
- **`crate::task::ProcReply`**: The message type transferred over the channel. It contains the transaction ID (`xid`) and the result of the procedure execution.
- **`crate::allocator::Buffer`**: A trait bound for the generic type `B`. It ensures that the buffers contained within `ProcReply` adhere to the memory management interface defined by the project's allocator.

---

## 2. Mechanics

**Intent:**
The module implements an asynchronous consumer task that bridges the gap between the internal result processing logic and the external network client. Its purpose is to continuously pull processed RPC results from a channel, serialize them into the ONC RPC format, and transmit them over a specific TCP connection.

**Inputs:**
- **Network Handle**: `OwnedWriteHalf` representing the connection to the client.
- **Result Stream**: `async_channel::Receiver<ProcReply<B>>` providing the results to be sent.

**Outputs:**
- **Network Traffic**: Bytes written to the `OwnedWriteHalf` socket.
- **Logs**: Error messages emitted to the tracing subsystem if serialization or writing fails.

**Steps:**
1. **Initialization**: The `WriteTask` is created via `new`, wrapping the network handle and the channel receiver.
2. **Spawning**: The `spawn` method is called, which moves the `WriteTask` into a new Tokio task. This task executes the `run` method.
3. **Serializer Setup**: Inside `run`, a `Serializer` instance is created, taking ownership of the `OwnedWriteHalf`.
4. **Event Loop**: The task enters an infinite loop calling `result_receiver.recv().await`.
5. **Message Processing**:
   - When a `ProcReply` is received, the task constructs an `OpaqueAuth` verifier. *Note: Currently, this is hardcoded to `AuthFlavor::None` with an empty body.*
   - The task calls `serializer.form_reply(reply, verifier).await`.
6. **Error Handling**:
   - If `form_reply` returns `Ok(())`, the reply is considered sent.
   - If `form_reply` returns `Err(e)`, the error is logged, and the loop continues. The connection remains open despite the error.
7. **Termination**: If the channel is closed (sender dropped), `recv` returns `Err`, and the loop terminates, dropping the task and the connection.

**Edge Cases:**
- **Channel Closure**: If all senders are dropped, `recv` returns an error, causing the task to exit gracefully.
- **Serialization/Write Failure**: If the socket is broken or serialization fails, the error is logged, but the task attempts to process subsequent messages. This behavior is explicitly marked with a `TODO` suggesting the connection should perhaps be closed on error.

**Complexity:**
- **Time**: Non-deterministic, dependent on the speed of the network I/O and the availability of messages in the channel.
- **Space**: O(1) auxiliary space, excluding the memory held by the `ProcReply` message being processed and the internal buffers of the `Serializer`.

**Determinism:**
- **Non-deterministic** regarding timing and message arrival order (though FIFO order is preserved by the channel). The logic flow is deterministic: every received message triggers a write attempt.

---

## 3. Dependency Mechanics

- **`Serializer` (from `nfs_mamont::serializer::server::serialize_struct`)**:
  - The `WriteTask` delegates the heavy lifting of protocol compliance to the `Serializer`. It relies on the `Serializer::new` method to accept the `OwnedWriteHalf` and on `Serializer::form_reply` to accept the high-level `ProcReply` and `OpaqueAuth`. The `WriteTask` assumes that `form_reply` handles the specifics of encoding the RPC header, accept status, and procedure-specific data. It passes the `OwnedWriteHalf` ownership to the serializer, implying the serializer manages the low-level writing strategy.

- **`ProcReply` (from `nfs_mamont::task`)**:
  - The `WriteTask` acts as a consumer for `ProcReply`. It relies on the structure of `ProcReply` (containing `xid` and `proc_result`) to provide the necessary payload for the `Serializer`. The generic `B: Buffer` parameter allows the task to handle replies containing memory allocated by the project's custom allocator without knowing the specific allocator implementation.

- **`OpaqueAuth` (from `nfs_mamont::rpc`)**:
  - The module uses `OpaqueAuth` to satisfy the interface requirements of the RPC protocol for replies. It currently bypasses the complexity of generating real cryptographic verifiers by constructing a dummy instance with `AuthFlavor::None`.

---

## 4. Data Model

**Entities:**
- **`WriteTask<B: Buffer>`**: The active agent responsible for writing replies. It holds the resources necessary to communicate with the network and the processing pipeline.

**Relations:**
- **`WriteTask` → `OwnedWriteHalf`** (Composition): The task owns the write half of the socket for the duration of its execution.
- **`WriteTask` → `async_channel::Receiver<ProcReply<B>>`** (Composition): The task owns the receiver end of the channel.
- **`WriteTask` → `Serializer`** (Transient Creation): The `run` method creates a `Serializer` instance, transferring ownership of the `OwnedWriteHalf` to it.

**Global Invariants:**
- **Runtime Requirement**: The `spawn` method must be called within a valid Tokio runtime context; otherwise, it will panic.
- **Channel Ownership**: The `Receiver` passed to `new` is moved into the task, ensuring that no other task can read from this specific channel endpoint concurrently.

---

## 5. Error Model

**Error Types:**
- **`std::io::Error`** (Implicit): Likely propagated from the `Serializer` or the underlying `OwnedWriteHalf` when network operations fail.
- **Serialization Errors**: Any errors generated by the `Serializer::form_reply` logic (e.g., encoding issues).

**Error Propagation Strategy:**
- **Logging**: Errors are caught in the `run` loop within a `match` statement. They are not propagated up the stack (as `run` is the entry point of an async task) but are logged using `tracing::error`.
- **Continuation**: Upon encountering an error, the system chooses to continue the loop rather than aborting the task.

**Recoverability:**
- **Partial**: The system attempts to recover from transient errors by continuing to process subsequent messages. However, if the network connection is fundamentally broken, subsequent writes will likely also fail, resulting in repeated error logs until the channel closes or the task is killed externally.

**Panics:**
- **Allowed**: Yes.
- **Conditions**:
  - In `spawn`: If called outside of a Tokio runtime context.

---

## 6. Traits

The module does not implement any public traits. It uses the following external traits:
- **`Buffer`** (from `crate::allocator`): Used as a bound `B: Buffer` to ensure the generic type provides the necessary memory access methods.

---

## 7. Overview

This module is used in order to **handle the output stage of the RPC request lifecycle**, specifically converting internal procedure results into network packets and sending them to a connected client. It solves the problem of decoupling the CPU-bound or I/O-bound work of processing an NFS request from the network latency of sending the response.

This system contains a **dedicated asynchronous writer task** (`WriteTask`) that acts as the sink for a channel of `ProcReply` messages. It integrates with the project's custom memory allocator (via the `Buffer` trait) and the ONC RPC serializer to ensure that responses are formatted correctly according to the protocol specification.

A typical usage scenario of the system involves a main server loop accepting a TCP connection. The connection is split into read and write halves. The write half is passed to `WriteTask::new` along with a channel receiver. Meanwhile, other tasks process incoming requests and send `ProcReply` results down the channel. The `WriteTask` picks up these replies, creates a (currently dummy) authentication verifier, and uses the `Serializer` to write the data to the socket.

Inside the system, the following things happen and they use this module:
1.  **Response Transmission**: When a request handler finishes processing (e.g., reading a file), it produces a `ProcReply`. This module ensures that reply is delivered to the client.
2.  **Protocol Encoding**: The module relies on `nfs_mamont::serializer::server::serialize_struct` to handle the binary encoding. The `WriteTask` itself is agnostic to the specific procedure being called; it simply passes the `ProcReply` to the serializer.
3.  **Error Containment**: By catching errors in the `run` loop and logging them, the module prevents a single failed reply from crashing the entire connection handler task, although this behavior is subject to change (as noted in the code TODOs).

The critical aspect of this module is the **separation of concerns**: it isolates the mechanics of writing to a specific TCP socket and formatting the RPC reply from the logic of generating the reply content.