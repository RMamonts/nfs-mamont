<!-- SPEC_HASH: 0ff30484263b852f552175ca6db2d2152ff1440ce183cded8cd982566076fa2c -->
# Module Specification

Module: nfs_mamont::task::connection
Rust File: src/task/connection/mod.rs

---

## 1. Dependencies

From the provided context and dependency facts:

- **`tokio::net::TcpStream`**: Represents the established network connection to a client. It is the primary resource that the module manages, splitting it into read and write halves.
- **`async_channel`**: Used to create an unbounded channel (`Sender`/`Receiver`) for `ProcReply<B>`. This channel bridges the gap between the read-side logic (which generates or dispatches requests) and the write-side logic (which serializes and sends responses), allowing them to operate concurrently.
- **`tracing`**: Used for structured logging. Specifically, `error!` is used to log failures when attempting to retrieve the peer address of the connected socket.
- **`crate::task::connection::read`**: Provides the `ReadTask` implementation. The module constructs and spawns this task to handle incoming RPC data, parsing, and request dispatching.
- **`crate::task::connection::write`**: Provides the `WriteTask` implementation. The module constructs and spawns this task to handle the serialization and transmission of RPC replies.
- **`crate::context::ServerContext`**: Acts as a dependency container. The module uses it to retrieve shared resources required by the `ReadTask`, specifically the write allocator and the sender for the VFS pool.
- **`crate::task::global::mount::MountCommand`**: The message type for the global MOUNT task channel. The module passes a sender for this type to the `ReadTask` so it can forward mount requests.
- **`crate::task::global::nlm::NlmCommand`**: The message type for the global NLM task channel. The module passes a sender for this type to the `ReadTask` so it can forward lock management requests.
- **`crate::vfs::Vfs`**: A trait bound for the generic parameter `V`. It ensures that the filesystem backend provided by the context is compatible with the VFS pool used by the connection tasks.
- **`crate::allocator::{Allocator, Buffer}`**: Traits defining the memory management strategy. The module uses these to ensure that the allocators provided by the context are compatible with the buffer types used in the RPC pipeline.

---

## 2. Mechanics

**Intent:**
The module serves as a factory for the lifecycle of a single NFS client connection. Its purpose is to take a raw TCP connection, split it into independent read and write streams, establish the communication channel linking them, and inject the necessary global dependencies (allocators, global task senders) to create a functional asynchronous request-response pipeline.

**Inputs:**
- **`socket: TcpStream`**: The active TCP connection accepted from a client.
- **`mount_sender: async_channel::Sender<MountCommand<B>>`**: Channel sender for the global MOUNT protocol handler.
- **`nlm_sender: async_channel::Sender<NlmCommand<B>>`**: Channel sender for the global NLM protocol handler.
- **`context: &ServerContext<A, V, B>`**: Reference to the server's shared state, providing access to the VFS pool and memory allocators.

**Outputs:**
- **Side Effects**: Spawns two asynchronous Tokio tasks (`ReadTask` and `WriteTask`) that take ownership of the connection halves and begin processing immediately.
- **Error Logging**: Logs an error if the peer address cannot be determined.

**Steps:**

1. **Peer Address Resolution**: Attempts to retrieve the remote socket address (`peer_addr`) from the `socket`.
   - If this fails (returns `Err`), the error is logged, and the function returns immediately, effectively closing the connection without spawning tasks.
2. **Stream Splitting**: Calls `socket.into_split()` to separate the TCP stream into an `OwnedReadHalf` and an `OwnedWriteHalf`. This allows concurrent reading and writing without synchronization locks on the stream itself.
3. **Channel Creation**: Creates a new unbounded `async_channel` pair `(result_sender, result_receiver)` for `ProcReply<B>`. This channel connects the read side (which initiates or receives replies) to the write side.
4. **Read Task Initialization**: Instantiates `read::ReadTask` with:
   - The `OwnedReadHalf`.
   - The `peer_addr`.
   - The global `mount_sender` and `nlm_sender`.
   - The `result_sender` (cloned) to send replies to the write task.
   - The write allocator from `context`.
   - The VFS pool sender from `context`.
5. **Read Task Spawning**: Calls `.spawn()` on the `ReadTask`, detaching it to run in the background.
6. **Write Task Initialization**: Instantiates `write::WriteTask` with:
   - The `OwnedWriteHalf`.
   - The `result_receiver`.
7. **Write Task Spawning**: Calls `.spawn()` on the `WriteTask`, detaching it to run in the background.

**Edge Cases:**
- **Connection Setup Failure**: If `socket.peer_addr()` fails, the function aborts. No tasks are spawned, and the socket is dropped (closed).

**Complexity:**
- **Time**: O(1). The function performs a constant number of operations (syscalls for peer addr, split, channel creation).
- **Space**: O(1) for the stack frame. The channel is unbounded, so heap usage grows dynamically if the producer (read side) outpaces the consumer (write side), but this is managed by the channel implementation, not this module.

**Determinism:**
- **Deterministic**. The setup logic is linear and conditional only on the success of `peer_addr`.

---

## 3. Dependency Mechanics

- **`ReadTask` (from `nfs_mamont::task::connection::read`)**:
  - **Protocol Demultiplexing**: The connection module relies on the `ReadTask` to handle the complexity of parsing incoming bytes and determining which global service (NFS, MOUNT, NLM) should handle the request.
  - **Dependency Injection**: The connection module acts as the injector, passing the specific `result_sender` and global task senders into the `ReadTask`. This ensures that the `ReadTask` is correctly wired into the broader server architecture.

- **`WriteTask` (from `nfs_mamont::task::connection::write`)**:
  - **Response Serialization**: The connection module relies on the `WriteTask` to consume `ProcReply` messages from the channel created in this module and handle the low-level details of writing them to the socket.

- **`ServerContext` (from `nfs_mamont::context`)**:
  - **Resource Access**: The module uses the context to decouple the connection creation logic from the specific configuration of the server's memory and worker pools. By calling `get_write_allocator()` and `get_vfs_pool()`, it retrieves the specific instances of allocators and channels that this connection must use.

- **`async_channel`**:
  - **Pipeline Coupling**: The module creates the specific channel instance that binds the `ReadTask` and `WriteTask` together. This unbounded channel allows the read side to offload work (or replies) immediately without waiting for the network write to complete, maximizing throughput.

---

## 4. Data Model

**Entities:**
- This module defines no public structs or enums. It acts as a constructor function.

**Relations:**
- **`TcpStream` → `ReadTask` & `WriteTask`**: The module transforms a single `TcpStream` into two separate ownership relationships (read half and write half).
- **`ReadTask` → `WriteTask`**: The module establishes a logical connection via the `async_channel` created in step 3. The `ReadTask` holds the `Sender`, and the `WriteTask` holds the `Receiver`.

**Global Invariants:**
- **Task Pairing**: For every successful call to `new`, exactly one `ReadTask` and one `WriteTask` are spawned.
- **Channel Wiring**: The `result_sender` given to the `ReadTask` must be the counterpart to the `result_receiver` given to the `WriteTask`.

---

## 5. Error Model

**Error Types:**
- **`std::io::Error`**: Implicitly returned by `socket.peer_addr()`.

**Error Propagation Strategy:**
- **Logging and Abort**: If `peer_addr` fails, the error is logged using `tracing::error!`, and the function returns `()`. The socket is dropped, closing the connection. The error is not propagated to the caller.

**Recoverability:**
- **Not Recoverable (per connection)**: If the connection setup fails (e.g., invalid socket state), the module does not retry; it simply abandons the connection.

**Panics:**
- **Allowed**: No explicit panics in this module. However, the spawned tasks (`ReadTask`, `WriteTask`) may panic if the Tokio runtime is not available or if internal invariants are violated.

---

## 6. Traits

The module does not define or implement any traits. It uses the following traits as generic bounds:
- **`A: Allocator<Buffer = B> + Send + Sync + 'static`**: Ensures the allocator can be shared across threads and used asynchronously.
- **`B: Buffer + 'static`**: Ensures the buffer type meets the interface requirements.
- **`V: Vfs<B> + Send + Sync + 'static`**: Ensures the filesystem backend is thread-safe and compatible with the buffer type.

---

## 7. Overview

This module is used in order to **initialize the processing pipeline for a single client connection** in an asynchronous NFS server. It solves the problem of coordinating the split between reading requests and writing responses, ensuring that both halves of the connection are correctly wired to the global services (Mount, NLM, VFS) and the memory management subsystem.

This system contains a **connection factory pattern** where the `new` function acts as the entry point for connection lifecycle management. It encapsulates the logic required to transform a raw `TcpStream` into a structured, multi-stage processing pipeline. By splitting the stream, it allows the server to handle reading and writing concurrently, which is crucial for performance in high-latency or high-throughput scenarios.

A typical usage scenario of the system involves the main server loop accepting a new TCP connection. The main loop calls `connection::new`, passing the socket and references to the global task channels and server context. The `new` function immediately splits the socket and spawns the two tasks. From that point on, the `ReadTask` continuously ingests RPC messages, dispatching work to the global VFS pool or specific protocol tasks, while the `WriteTask` independently waits for results on the channel and pushes bytes back to the client.

Inside the system, the following things happen and they use this module:
1. **Resource Injection**: The module is responsible for passing the specific `write_allocator` and `vfs_pool` sender to the `ReadTask`. This ensures that the connection uses the memory pools and worker queues defined by the server's configuration.
2. **Channel Topology**: The module creates the local `result_sender`/`result_receiver` pair. This topology is critical because it allows the `ReadTask` (or the global workers it communicates with) to send replies directly to the `WriteTask` without the `ReadTask` having to wait for the write operation to complete, thereby decoupling ingress from egress.
3. **Error Containment**: By handling the `peer_addr` failure locally and logging it, the module prevents a bad connection from crashing the server or propagating errors up to the main accept loop, allowing the server to continue accepting new clients.

The critical aspect of this module is the **orchestration of the connection split and task wiring**, establishing the necessary infrastructure for the `ReadTask` and `WriteTask` to operate independently yet cohesively.