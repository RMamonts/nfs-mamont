<!-- SPEC_HASH: f3a77b96551d4dcd1333f05a9a4063328cfd9357a59b44cd3aefcd72201b588b -->
# Module Specification

Module: nfs_mamont
Rust File: src/lib.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::sync::Arc`**: Used to wrap the `Mount` and `Nlm` service implementations. This allows the services to be shared safely between the main server loop and the spawned global tasks (`MountTask`, `NlmTask`) without transferring ownership.
- **`tokio::net::TcpListener`**: Used to bind to a network address and accept incoming TCP connections from NFS clients.
- **`tracing_subscriber`**: Used to initialize the global tracing subscriber for structured logging. It configures the log level (filter) based on environment variables or a default.
- **`crate::task::global::mount::MountTask`**: Used to create the global worker responsible for processing MOUNT protocol requests. The module spawns this task once to handle mount operations for all clients.
- **`crate::task::global::nlm::NlmTask`**: Used to create the global worker responsible for processing NLM (Network Lock Manager) protocol requests. The module spawns this task once to handle file locking operations.
- **`crate::task::connection`**: Used to handle the lifecycle of individual client connections. The module calls `connection::new` for every accepted socket to spawn the read and write tasks.
- **`crate::context::ServerContext`**: Used as a dependency container holding the VFS (Virtual File System) worker pool, memory allocators, and the filesystem backend. It is passed to every connection handler.
- **`crate::vfs::Vfs`**: A trait bound for the generic parameter `V`. It ensures that the filesystem backend provided by the context is compatible with the server's operations.
- **`crate::mount::Mount`**: A trait bound for the generic parameter `M`. It defines the interface for the MOUNT protocol service.
- **`crate::nlm::Nlm`**: A trait bound for the generic parameter `N`. It defines the interface for the NLM protocol service.
- **`crate::allocator`**: Re-exports `Allocator`, `Buffer`, `Impl`, `Slice`, and `UnownedBuffer` to make them available at the crate root for users configuring the server.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- **`init_tracing`**: To configure and initialize the global logging infrastructure for the application, ensuring that debug information is available by default unless overridden by environment variables.
- **`handle_forever`**: To act as the main server entry point. It orchestrates the startup sequence by initializing global protocol handlers (MOUNT and NLM) and entering an infinite loop to accept and process incoming TCP connections.

Inputs:
- **`init_tracing`**: Reads the `RUST_LOG` environment variable (implicitly via `try_from_default_env`).
- **`handle_forever`**:
 - `listener`: A bound `TcpListener` ready to accept connections.
 - `context`: A `ServerContext` containing shared resources (VFS pool, allocators).
 - `mount_service`: An `Arc` to a service implementing the `Mount` trait.
 - `nlm_service`: An `Arc` to a service implementing the `Nlm` trait.

Outputs:
- **`init_tracing`**: Side effect of setting the global tracing subscriber. Returns `()`.
- **`handle_forever`**: Returns `std::io::Result<()>` which is `Err` if accepting a connection fails, or otherwise never returns (loops forever). Side effects include spawning global tasks and connection tasks.

Steps:
1. **Initialization (`init_tracing`)**:
 - Attempts to construct an `EnvFilter` from the environment. If that fails, it defaults to `nfs_mamont=debug`.
 - Attempts to initialize a global `fmt` subscriber with this filter. If a subscriber is already set, the error is ignored.

2. **Server Startup (`handle_forever`)**:
 - Instantiates `MountTask` using the provided `mount_service`.
 - Spawns the `MountTask` onto the Tokio runtime.
 - Instantiates `NlmTask` using the provided `nlm_service`.
 - Spawns the `NlmTask` onto the Tokio runtime.

3. **Connection Loop (`handle_forever`)**:
 - Enters a `loop` calling `listener.accept().await`.
 - Upon accepting a socket `(socket, _)`:
 - Calls `connection::new(socket, mount_sender.clone(), nlm_sender.clone(), &context).await`.
 - This spawns the read and write tasks for this specific connection, wiring them to the global tasks and the server context.

Edge Cases:
- **Logging Initialization**: If `init_tracing` is called multiple times, subsequent calls do nothing (error is ignored).
- **Accept Failure**: If `listener.accept()` returns an `Err`, the `?` operator propagates the error, terminating the `handle_forever` function and stopping the server.

Complexity:
- **Time**:
 - `init_tracing`: O(1).
 - `handle_forever`: O(1) per connection setup. The loop runs indefinitely.
- **Space**: O(1) stack space. Heap usage grows with the number of active connections and tasks spawned.

Determinism:
- **Non-deterministic**. The order of connection acceptance and the timing of async task execution depend on the network and the Tokio scheduler.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::task::global::mount`**:
 - **Global Dispatching**: The module relies on `MountTask` to serialize MOUNT protocol requests. By creating the task once and passing its `Sender` to every connection, the module ensures that all mount operations are processed through a single, consistent point of control.
- **From `nfs_mamont::task::global::nlm`**:
 - **Global Locking**: Similar to the MOUNT task, the module relies on `NlmTask` to handle file locking. This centralizes the lock state management, preventing race conditions across different client connections.
- **From `nfs_mamont::task::connection`**:
 - **Connection Factory**: The module uses `connection::new` to encapsulate the complexity of setting up a client session. It delegates the splitting of the TCP stream and the spawning of read/write tasks to this module, passing the necessary global senders (`mount_sender`, `nlm_sender`) and context to link the connection to the rest of the server.
- **From `nfs_mamont::context`**:
 - **Resource Injection**: The module passes the `ServerContext` to every connection. This ensures that all connections have access to the same VFS worker pool and memory allocators, enforcing the server's resource limits and concurrency model.

---

## 4. Data Model

Entities:
- This module defines no new public structs or enums. It acts as an orchestrator and a re-export namespace.

Relations:
- **`handle_forever` → `MountTask` (1:1)**: The main loop owns the lifecycle of the single global `MountTask`.
- **`handle_forever` → `NlmTask` (1:1)**: The main loop owns the lifecycle of the single global `NlmTask`.
- **`handle_forever` → `connection::new` (1:N)**: The main loop invokes the connection factory for every accepted client.

Global Invariants:
- The `mount_sender` and `nlm_sender` passed to `connection::new` must be valid clones of the senders associated with the spawned `MountTask` and `NlmTask`.

## 5. Error Model

Error Types:
- **`std::io::Error`**: Returned by `listener.accept()`.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used in `handle_forever`. If accepting a connection fails, the error is returned to the caller, terminating the server loop.

Recoverability:
- **Not Recoverable (Server Level)**: If `listener.accept()` fails catastrophically (e.g., socket closed), the server stops. Individual connection errors inside `connection::new` are handled by that module (e.g., logging and dropping the socket) and do not stop the loop.

Panics:
- **Allowed**: No explicit panics in this module. However, panics in spawned tasks (`MountTask`, `NlmTask`, or connection tasks) will occur independently and may be observed by the Tokio runtime.

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **bootstrap and orchestrate the lifecycle of a multi-protocol NFS server**. It serves as the root of the dependency tree, tying together the network layer, protocol handlers, and filesystem abstraction into a cohesive executable unit.

The system contains a complex architecture designed to handle high-concurrency file serving. It separates concerns into global tasks (for MOUNT and NLM protocols, which require serialized state) and per-connection tasks (for NFS data transfer). The `handle_forever` function is the critical glue that initializes this topology. It ensures that the global services are running before any connections are accepted and that every new connection is immediately wired into the existing infrastructure (global task channels and the VFS pool).

A typical usage scenario of the system involves a user (or a binary main function) configuring the necessary components: a `TcpListener`, a filesystem backend (implementing `Vfs`), memory allocators, and protocol services (implementing `Mount` and `Nlm`). The user then calls `handle_forever`. From that point on, the module takes over: it starts the background workers for locking and mounting, and then enters a loop to accept clients. For each client, it spawns a dedicated pipeline (`connection::new`) that reads RPC requests, dispatches them to the appropriate global worker or VFS pool, and writes responses back.

Inside the system, the following things happen and they use this module:
1. **Service Initialization**: The module is responsible for the "start-up" phase. It creates the `MountTask` and `NlmTask`, which are not accessible or usable by the connection handlers until this module creates them and distributes their channel senders.
2. **Dependency Injection**: The module acts as the injector for the `ServerContext`. By passing the context to `connection::new`, it ensures that every connection uses the same configured memory pools and worker threads, enforcing the server's resource constraints.
3. **Protocol Demultiplexing**: While the actual parsing happens in the connection tasks, this module establishes the *paths* for that demultiplexing by providing the specific `mount_sender` and `nlm_sender` that the connection tasks will use to forward requests.

The critical aspect of this module is the **centralization of the server's entry point**. It abstracts away the complexity of spawning tasks and managing channels, providing a simple `async` function that runs the server "forever".