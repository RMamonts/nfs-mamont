<!-- SPEC_HASH: 0ff30484263b852f552175ca6db2d2152ff1440ce183cded8cd982566076fa2c -->
# Module Specification

Module: nfs_mamont::task::connection
Rust File: src/task/connection/mod.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::net::TcpStream`**: Used as the input raw network connection representing a client session. It is split into read and write halves to allow concurrent processing of incoming and outgoing data.
- **`tracing::error`**: Used to log errors when the system fails to determine the peer address of the connected socket, which is a prerequisite for further processing.
- **`async_channel`**: Used to create an unbounded channel (`Sender<ProcReply<B>>`, `Receiver<ProcReply<B>>`). This channel serves as the dedicated communication link between the read task (producer of replies) and the write task (consumer of replies) for this specific connection.
- **`crate::allocator::{Allocator, Buffer}`**: Used as generic bounds (`A: Allocator<Buffer = B>`, `B: Buffer`) to ensure that the connection tasks can interact with the server's memory management system for buffer allocation.
- **`crate::context::ServerContext`**: Used as a dependency container. It provides access to the shared VFS worker pool (via `get_vfs_pool`) and the write-side memory allocator (via `get_write_allocator`), which are required by the read task for parsing and dispatching.
- **`crate::task::global::mount::MountCommand`**: Used as the message type for the channel sender (`mount_sender`) that is passed to the read task. This allows the read task to forward MOUNT protocol requests to the global mount handler.
- **`crate::task::global::nlm::NlmCommand`**: Used as the message type for the channel sender (`nlm_sender`) that is passed to the read task. This allows the read task to forward NLM protocol requests to the global NLM handler.
- **`crate::task::ProcReply`**: Used as the payload type for the internal channel connecting the read and write tasks. It represents the standardized result of an RPC operation.
- **`crate::vfs::Vfs`**: Used as a trait bound (`V: Vfs<B>`) for the `ServerContext`, ensuring that the backend provided by the context supports the necessary file system operations.
- **`crate::task::connection::read`**: A private submodule providing the `ReadTask`. This module instantiates and spawns it to handle the ingress side of the connection.
- **`crate::task::connection::write`**: A private submodule providing the `WriteTask`. This module instantiates and spawns it to handle the egress side of the connection.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Connection Pipeline Initialization (`new`)

**Intent:**
To act as a factory for the per-connection asynchronous processing pipeline. It isolates the setup logic—splitting the socket, creating internal channels, and injecting global dependencies—into a single entry point, ensuring that every connection is initialized with a consistent and correctly wired architecture.

**Inputs:**
- `socket: TcpStream`: The accepted network connection.
- `mount_sender: async_channel::Sender<MountCommand<B>>`: Channel to the global MOUNT task.
- `nlm_sender: async_channel::Sender<NlmCommand<B>>`: Channel to the global NLM task.
- `context: &ServerContext<A, V, B>`: Shared server context containing the VFS pool and allocators.

**Outputs:**
- Returns `()` (unit).
- Side effects: Spawns two asynchronous tasks (`ReadTask` and `WriteTask`) that take ownership of the connection resources.

**Steps:**
1. **Peer Resolution**: Attempts to retrieve the remote peer address using `socket.peer_addr()`. If this fails, an error is logged and the function returns immediately, dropping the socket.
2. **Stream Splitting**: Calls `socket.into_split()` to separate the TCP stream into an `OwnedReadHalf` and an `OwnedWriteHalf`. This allows the read and write tasks to operate concurrently without locking.
3. **Internal Channel Creation**: Creates a new unbounded `async_channel` of type `ProcReply<B>`. This results in a `result_sender` and a `result_receiver`.
4. **Read Task Instantiation**:
 - Constructs `read::ReadTask::new` passing:
 - The `readhalf`.
 - The `peer_addr`.
 - The global `mount_sender` and `nlm_sender`.
 - The `result_sender` (cloned).
 - The `write_allocator` from `context`.
 - The `vfs_pool` sender from `context`.
5. **Read Task Spawning**: Calls `.spawn()` on the `ReadTask`, detaching it to run on the Tokio runtime.
6. **Write Task Instantiation**:
 - Constructs `write::WriteTask::new` passing:
 - The `writehalf`.
 - The `result_receiver`.
7. **Write Task Spawning**: Calls `.spawn()` on the `WriteTask`, detaching it to run on the Tokio runtime.

**Edge Cases:**
- **Peer Address Failure**: If `socket.peer_addr()` returns an `Err`, the connection is aborted before any tasks are spawned. This prevents processing anonymous or malformed connections.

**Complexity:**
- **Time**: O(1). The operations involved (splitting socket, creating channels) are constant time.
- **Space**: O(1) stack space. The heap usage is determined by the buffer sizes of the created channels, which are fixed at creation.

**Determinism:**
- **Deterministic**. The logic follows a strict sequence of setup steps. The behavior is fully defined by the inputs and the success of system calls (like `peer_addr`).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::task::connection::read`**:
 - **`ReadTask::new` and `spawn`**: The connection module relies on the `ReadTask`'s constructor to accept the specific wiring (channels, allocators, context) and its `spawn` method to detach the execution logic. The `ReadTask` implements the complex logic of parsing RPC messages and dispatching them to the global tasks or the local result channel.
 - **`result_sender` Usage**: The connection module passes the sender end of the internal channel to the `ReadTask`. The `ReadTask` uses this to send `ProcReply` messages (either generated locally for NULL procedures or errors) to the `WriteTask`.

- **From `nfs_mamont::task::connection::write`**:
 - **`WriteTask::new` and `spawn`**: The connection module relies on the `WriteTask` to consume messages from the internal channel and write them to the network. The `WriteTask` handles serialization and I/O errors.
 - **`result_receiver` Usage**: The connection module passes the receiver end of the internal channel to the `WriteTask`. This creates the unidirectional flow of data from the read side (or global workers) to the write side.

- **From `nfs_mamont::context`**:
 - **`get_vfs_pool` and `get_write_allocator`**: The connection module extracts these specific resources from the `ServerContext`. The `vfs_pool` sender is passed to the `ReadTask` to dispatch NFS requests, and the `write_allocator` is passed to the `ReadTask` to allocate memory for parsing incoming data.

- **From `nfs_mamont::task::global::mount` and `nfs_mamont::task::global::nlm`**:
 - **Global Senders**: The connection module receives senders for the global `MountTask` and `NlmTask` as arguments. It forwards these to the `ReadTask`, enabling the connection to participate in the global protocol handling without managing that state itself.

---

## 4. Data Model

Entities:
- This module defines no public structs or enums. It acts as a constructor function.

Relations:
- **`new` function → `ReadTask` (Factory)**: Creates and configures the read task.
- **`new` function → `WriteTask` (Factory)**: Creates and configures the write task.
- **`ReadTask` ↔ `WriteTask` (Channel)**: The `new` function establishes an indirect relationship between these two tasks via the `async_channel` it creates. The `ReadTask` holds the `Sender`, and the `WriteTask` holds the `Receiver`.

Global Invariants:
- **Channel Pairing**: The `result_sender` passed to `ReadTask` must be the originating sender of the `result_receiver` passed to `WriteTask`. This ensures that replies generated by the read task (or forwarded from global tasks) are actually received by the write task for the same connection.

## 5. Error Model

Error Types:
- **`std::io::Error`**: Implicitly returned by `socket.peer_addr()`.

Error Propagation Strategy:
- **Early Termination**: If `peer_addr()` fails, the error is logged using `tracing::error!`, and the function returns immediately. The `TcpStream` is dropped, closing the connection.

Recoverability:
- **Not Recoverable (Per Connection)**: If the peer address cannot be determined, the module assumes the connection is invalid or unusable and terminates it immediately. No retry logic is implemented at this level.

Panics:
- **Allowed**: Indirectly.
- **Conditions**:
 - If `ReadTask::spawn` or `WriteTask::spawn` is called outside of a Tokio runtime, the task will panic (as per `tokio::spawn` documentation).
 - If the internal channel creation fails (unlikely for `unbounded` unless OOM), it might panic or propagate an error depending on the `async_channel` implementation details (though typically infallible for unbounded in this context).

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **encapsulate the lifecycle initialization of a client connection pipeline** within the NFS-Mamont server. It solves the architectural problem of wiring together the network I/O, the protocol parsing, the global service dispatchers, and the response serialization into a cohesive, isolated unit for every TCP connection accepted by the server.

The system contains a **concurrent, multi-stage processing architecture**. The `connection` module acts as the "bootstrapper" for this architecture on a per-connection basis. It takes the raw `TcpStream` and the global context (which holds references to the VFS pool and global protocol tasks) and constructs a closed loop of data flow:
1.  **Ingress**: The `ReadTask` reads bytes, parses them, and either handles them locally (NULL procs) or forwards them to global tasks (MOUNT, NLM) or the VFS pool.
2.  **Communication**: An internal `async_channel` connects the producers of results (ReadTask, Global Tasks, VFS) to the consumer of results (WriteTask).
3.  **Egress**: The `WriteTask` takes results from the channel, serializes them, and writes them to the socket.

A typical usage scenario of the system involves the main server loop (`lib.rs::handle_forever`) accepting a new TCP socket. It calls `connection::new`, passing the socket and the global senders/context. The `connection` module then splits the socket, sets up the private channel for replies, and spawns the two tasks. From that moment on, the main loop is free to accept the next connection, while the spawned tasks handle the client's requests autonomously.

Inside the system, the following things happen and they use this module:
- **Resource Isolation**: By creating a specific `async_channel` for every connection, the module ensures that replies for Client A never end up being sent to Client B, even though they might be processed by the same global VFS worker.
- **Dependency Injection**: The module extracts the necessary components (VFS pool sender, allocators) from the `ServerContext` and passes them down to the `ReadTask`. This ensures that the connection tasks adhere to the global memory limits and concurrency settings defined at server startup.
- **Error Containment**: If the connection setup fails (e.g., getting the peer address), the module handles the cleanup (dropping the socket) before any tasks are spawned, preventing "zombie" tasks from running on invalid resources.

The critical aspect of this module is the **orchestration of the split-stream pattern**. It enables full-duplex network communication (reading and writing simultaneously) by separating the `TcpStream` into two owned halves and assigning each half to a dedicated asynchronous task, linked only by the asynchronous channel.