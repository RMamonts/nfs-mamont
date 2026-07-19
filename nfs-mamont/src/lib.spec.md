<!-- SPEC_HASH: f3a77b96551d4dcd1333f05a9a4063328cfd9357a59b44cd3aefcd72201b588b -->
# Module Specification

Module: nfs_mamont
Rust File: src/lib.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::net::TcpListener`**: Used as the source of incoming TCP connections. The `handle_forever` function takes ownership of this listener to continuously accept new client sockets in an asynchronous loop.
- **`tracing_subscriber`**: Used to initialize the global tracing subscriber. The `init_tracing` function configures the logging framework to filter and output log events based on environment variables or a default debug level.
- **`crate::task::global::mount::MountTask`**: Used to instantiate and spawn the global actor responsible for handling MOUNT protocol requests. This ensures that mount state is managed serially across all connections.
- **`crate::task::global::nlm::NlmTask`**: Used to instantiate and spawn the global actor responsible for handling Network Lock Manager (NLM) protocol requests. This ensures that lock state is managed serially.
- **`crate::task::connection`**: Used to handle the lifecycle of individual client connections. The `connection::new` function is called for every accepted socket to spawn the read and write tasks.
- **`crate::context::ServerContext`**: Used as a container for shared server resources (VFS pool, allocators). It is passed to connection tasks to provide access to these shared resources.
- **`crate::vfs::Vfs`**: Used as a trait bound (`V: Vfs<B>`) to ensure that the backend provided in the context supports the full suite of NFSv3 file system operations.
- **`crate::mount::Mount`**: Used as a trait bound (`M: Mount`) to ensure the provided service implements the MOUNT protocol logic.
- **`crate::nlm::Nlm`**: Used as a trait bound (`N: Nlm`) to ensure the provided service implements the NLM protocol logic.
- **`crate::allocator::Allocator`**: Used as a trait bound (`A: Allocator`) to ensure the memory management strategy provided by the context can allocate buffers.
- **`crate::allocator::Buffer`**: Used as a trait bound (`B: Buffer`) and associated type to define the memory unit used for data transfer.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Tracing Initialization (`init_tracing`)

**Intent:**
To configure the global logging infrastructure for the NFS server. It sets up the `tracing` subscriber to filter log messages based on environment variables (e.g., `RUST_LOG`) or defaults to a debug level for the `nfs_mamont` crate if no environment configuration is present.

**Inputs:**
- None (reads environment variables implicitly).

**Outputs:**
- Side effect: Registers the global tracing subscriber.

**Steps:**
1. Attempts to construct an `EnvFilter` from the default environment variables.
2. If the environment variables are missing or invalid, it falls back to a default filter: `nfs_mamont=debug`.
3. Initializes the `fmt` subscriber with the calculated filter.

**Edge Cases:**
- **Release Builds**: The documentation comment states "In release builds this is a no-op," implying that the `init_tracing` function might be conditionally compiled or called in a way that has no effect in release mode, despite the code logic suggesting a default debug filter. *Uncertainty: It is unclear if the code provided is the full source or if `#[cfg(debug_assertions)]` attributes were omitted in the snippet.*

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic (behavior depends solely on environment variables).

### Mechanism 2: Server Orchestration Loop (`handle_forever`)

**Intent:**
To act as the main entry point for the NFS server's runtime lifecycle. It initializes the global protocol actors (MOUNT and NLM) and enters an infinite loop to accept and process incoming TCP connections, delegating the per-connection logic to the `connection` module.

**Inputs:**
- `listener: TcpListener`: The bound TCP socket ready to accept connections.
- `context: ServerContext<A, V, B>`: Shared context containing the VFS pool and allocators.
- `mount_service: Arc<M>`: Thread-safe reference to the MOUNT protocol implementation.
- `nlm_service: Arc<N>`: Thread-safe reference to the NLM protocol implementation.

**Outputs:**
- `std::io::Result<()>`: Returns an I/O error if the listener fails, otherwise runs indefinitely.

**Steps:**
1. **Global Task Initialization**:
 - Calls `MountTask::new(mount_service)` to create the MOUNT actor and retrieve a channel sender.
 - Calls `.spawn()` on the `MountTask` to start it on the Tokio runtime.
 - Calls `NlmTask::new(nlm_service)` to create the NLM actor and retrieve a channel sender.
 - Calls `.spawn()` on the `NlmTask` to start it on the Tokio runtime.
2. **Accept Loop**:
 - Enters a `loop` block.
 - Awaits `listener.accept()`. If it returns an `Err`, the function returns the error immediately.
 - Destructures the `Ok((socket, _addr))`.
 - Calls `connection::new(socket, mount_sender.clone(), nlm_sender.clone(), &context).await` to handle the new connection.

**Edge Cases:**
- **Listener Failure**: If `listener.accept()` returns an `Err`, the server loop terminates, and the error is propagated to the caller.
- **Connection Task Failure**: If `connection::new` encounters an error (e.g., peer address resolution failure), it handles it internally (by logging/dropping the socket), and the main loop continues to accept new connections.

**Complexity:**
- Time: O(N) where N is the number of connections accepted.
- Space: O(N) for the accumulated connection tasks + O(1) for the global tasks.

**Determinism:**
- Non-deterministic. The order of connection acceptance and the scheduling of asynchronous tasks depend on the OS network stack and the Tokio runtime scheduler.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::task::global::mount`**:
 - **`MountTask::new` and `spawn`**: The module relies on the constructor to create the actor instance and the `spawn` method to detach it. This allows the main loop to delegate all MOUNT protocol handling to a single, serialized background task.
 - **`Sender<MountCommand<B>>`**: The module uses the sender returned by `new` to forward MOUNT requests from individual connections to the global task.

- **From `nfs_mamont::task::global::nlm`**:
 - **`NlmTask::new` and `spawn`**: Similar to the MOUNT task, the module uses these to start the global NLM actor, ensuring centralized lock management.
 - **`Sender<NlmCommand<B>>`**: The module uses the sender to forward NLM requests from connections to the global task.

- **From `nfs_mamont::task::connection`**:
 - **`connection::new`**: The module uses this function to bootstrap the per-connection processing pipeline. It passes the raw socket, the global senders (for MOUNT/NLM), and the server context (for VFS/allocators) to this function, effectively offloading the connection's entire lifecycle to it.

- **From `nfs_mamont::context`**:
 - **`ServerContext`**: The module treats this as a dependency injection container. It passes a reference to this context to every new connection, ensuring that all connections share the same VFS worker pool and memory allocators.

---

## 4. Data Model

Entities:
- This module defines no public structs or enums. It acts as a coordinator using types defined in its dependencies.

Relations:
- **`handle_forever` → `TcpListener` (Ownership)**: Takes ownership of the listener to accept connections.
- **`handle_forever` → `MountTask` (Ownership)**: Creates and owns the task instance until it is spawned.
- **`handle_forever` → `NlmTask` (Ownership)**: Creates and owns the task instance until it is spawned.
- **`handle_forever` → `connection::new` (Invocation)**: Calls the connection factory for every accepted socket.

Global Invariants:
- **Global Task Uniqueness**: The `handle_forever` function ensures that exactly one `MountTask` and one `NlmTask` are spawned for the lifetime of the server.
- **Sender Sharing**: The `Sender` handles for the global tasks are cloned and passed to every connection, ensuring all connections can communicate with the global actors.

## 5. Error Model

Error Types:
- **`std::io::Error`**: Returned by `listener.accept()`.

Error Propagation Strategy:
- **Early Termination**: If `listener.accept()` returns an `Err`, the `handle_forever` function returns this error immediately, stopping the server.

Recoverability:
- **Not Recoverable (Server Level)**: If the listener fails (e.g., socket closed), the server loop exits.
- **Recoverable (Connection Level)**: Failures within `connection::new` (e.g., getting peer address) are handled internally by that module and do not propagate to `handle_forever`, allowing the server to continue accepting other connections.

Panics:
- **Allowed**: Indirectly.
- **Conditions**:
 - If `MountTask::spawn` or `NlmTask::spawn` is called outside of a Tokio runtime.
 - If the `connection::new` task panics (propagates if not caught).

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **orchestrate the startup and runtime execution of the NFS-Mamont server**. It serves as the root of the library's logic, tying together the various subsystems (memory allocation, virtual filesystem, protocol handlers, and network I/O) into a cohesive, running service.

The system contains a complex, multi-protocol asynchronous server. The `lib.rs` module is necessary because it defines the sequence of operations required to transition from a configured set of services (VFS, MOUNT, NLM) to an active server listening on a network port. It solves the architectural problem of "wiring": ensuring that global state actors are started before connections are accepted, and that every connection is injected with the correct handles to communicate with those global actors.

A typical usage scenario of the system involves a `main` function (likely in `bin/nfs_mamont.rs` or similar) that constructs the services and allocators. It then calls `init_tracing()` to set up logging. Finally, it calls `handle_forever`, passing the bound TCP listener and the services. The `handle_forever` function then starts the background tasks for MOUNT and NLM protocols and enters the accept loop. When a client connects, `handle_forever` spawns a connection pipeline, passing the senders for the global tasks. This allows the connection to dispatch MOUNT and NLM requests to the centralized actors while handling NFS requests via the VFS pool provided in the context.

Inside the system, the following things happen and they use this module:
1. **Bootstrap Sequence**: The module enforces the order of operations: logging first, then global tasks, then the accept loop. This prevents race conditions where connections might be accepted before the global services are ready to receive requests.
2. **Resource Injection**: By passing the `ServerContext` to `connection::new`, the module ensures that every connection utilizes the shared memory pools and VFS workers, enforcing global resource limits and consistency.
3. **Protocol Demultiplexing**: The module sets up the infrastructure (via the global task senders) that allows the connection layer to demultiplex incoming RPC requests: MOUNT requests go to the `MountTask`, NLM requests go to the `NlmTask`, and NFS requests go to the `VfsPool` inside the context.

The critical aspect of this module is the **centralization of control flow**. It acts as the "conductor" of the server, ensuring that the asynchronous components start in the correct order and that the main loop efficiently delegates connection handling to specialized sub-modules.

**Uncertainty**: The comment in `init_tracing` states "In release builds this is a no-op," but the code implementation `EnvFilter::new("nfs_mamont=debug")` suggests it would default to debug logging regardless of the build profile unless `tracing` macros are configured to strip logs at compile time. The actual behavior in release builds depends on external configuration not fully visible in this snippet.