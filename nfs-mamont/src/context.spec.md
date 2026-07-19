<!-- SPEC_HASH: 82c53027039b9101eb38a6495af80d5edf87c927558590bf70702e3f9b730b31 -->
# Module Specification

Module: nfs_mamont::context
Rust File: src/context.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::sync::Arc`**: Used to wrap the backend and allocators. This allows multiple parts of the system (specifically the `VfsPool` and connection handlers) to share ownership of these resources without transferring ownership or risking use-after-free bugs.
- **`std::num::NonZeroUsize`**: Used to ensure that the `vfs_pool_size` passed to the constructor is strictly greater than zero, preventing the creation of an empty worker pool which would stall the server.
- **`crate::allocator`**: Used to define the generic bounds `A: Allocator` and `B: Buffer`. This allows the context to manage memory resources abstractly, supporting different memory management strategies (e.g., pooled vs. heap) without being coupled to a specific implementation.
- **`crate::task::global::vfs`**: Used to provide the `VfsPool` type. The context initializes this pool to manage the asynchronous execution of NFS procedures.
- **`crate::vfs`**: Used to define the `Vfs` trait bound `V: vfs::Vfs<B>`. This ensures that the backend provided to the context implements the full suite of NFSv3 filesystem operations required by the worker pool.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To act as a centralized dependency injection container for the server's shared resources.
- To encapsulate the configuration of the concurrency model (via `VfsPool`) and memory management (via `Allocators`) into a single object that can be passed to connection handlers.
- To ensure that all connections utilize the same underlying filesystem backend and memory pools, enforcing global resource limits and consistency.

Inputs:
- **`backend: Arc<V>`**: A shared reference to the filesystem implementation.
- **`read_allocator: Arc<A>`**: A shared reference to the allocator used for read buffers (incoming data).
- **`write_allocator: Arc<A>`**: A shared reference to the allocator used for write buffers (outgoing data).
- **`vfs_pool_size: NonZeroUsize`**: The number of worker threads to spawn in the VFS pool.

Outputs:
- **`ServerContext<A, V, B>`**: A struct holding the initialized worker pool, allocators, and backend.

Steps:
1. **Initialization (`new`)**:
 - The function accepts the shared resources and pool size.
 - It instantiates `VfsPool::new`, passing the pool size, a clone of the `backend`, and a clone of the `read_allocator`. This creates the worker pool that will execute filesystem operations.
 - It constructs the `ServerContext` struct, storing the newly created `vfs_pool`, the original `read_allocator`, the `write_allocator`, and the `backend`.
2. **Accessors (`get_...`)**:
 - The struct provides getter methods (`get_vfs_pool`, `get_backend`, `get_read_allocator`, `get_write_allocator`).
 - These methods return references or clones of the internal `Arc`s, allowing callers (like connection tasks) to acquire handles to the shared resources.

Edge Cases:
- **Pool Size**: The use of `NonZeroUsize` guarantees that the VFS pool will always have at least one worker. If zero were passed, the constructor would fail at the call site (or require unsafe handling), but the type system prevents this here.

Complexity:
- **Time**: O(1) for construction (involves only `Arc` cloning and struct initialization).
- **Space**: O(1) additional space on top of the resources passed in (the struct itself is just a few pointers).

Determinism:
- **Deterministic**. The construction and access logic are straightforward and have no side effects other than incrementing `Arc` reference counts.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::task::global::vfs`**:
 - **Worker Pool Initialization**: The module relies on `VfsPool::new` to set up the execution environment for NFS requests. The context configures this pool with a specific size and injects the necessary dependencies (backend, allocator) into it. This mechanism decouples the *definition* of the worker pool from its *configuration*.
- **From `nfs_mamont::vfs`**:
 - **Backend Abstraction**: The module holds an `Arc<V>` where `V: Vfs`. This allows the context to be agnostic to the specific filesystem implementation (e.g., local disk, in-memory, networked). The context simply passes this trait object to the pool, which invokes methods on it.
- **From `nfs_mamont::allocator`**:
 - **Memory Segregation**: The module holds two distinct allocator instances (`read_allocator` and `write_allocator`). This allows the system to potentially enforce different memory limits or strategies for reading incoming network data versus writing outgoing data, or simply to provide a unified access point to the same pool if configured identically.

---

## 4. Data Model

Entities:
- **`ServerContext<A, V, B>`**: The central context struct.
 - `vfs_pool: VfsPool<B>`: The pool of workers executing NFS procedures.
 - `read_allocator: Arc<A>`: Allocator for buffers used in read operations.
 - `write_allocator: Arc<A>`: Allocator for buffers used in write operations.
 - `backend: Arc<V>`: The filesystem implementation.

Relations:
- **Composition**: `ServerContext` owns `VfsPool`, `Arc<A>`, and `Arc<V>`.
- **Association**: The `vfs_pool` internally holds references to the `backend` and `read_allocator` (passed during construction), creating a shared ownership graph where the context and the pool both point to the same resources.

Global Invariants:
- **Shared State**: The `backend` and allocators returned by `get_backend`, `get_read_allocator`, and `get_write_allocator` point to the same underlying instances as those held by the `vfs_pool`. This ensures that memory accounting and filesystem state are consistent across the server.

## 5. Error Model

Error Types:
- None defined in this module.

Error Propagation Strategy:
- N/A (This module performs no I/O or fallible operations itself).

Recoverability:
- N/A.

Panics:
- **Allowed**: No explicit panics. However, if the `VfsPool::new` constructor panics (e.g., due to inability to spawn threads), the panic will propagate through `ServerContext::new`.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **centralize the management of shared server resources**, specifically the filesystem backend, memory allocators, and the asynchronous worker pool. It solves the architectural problem of dependency propagation: instead of passing individual references to the backend, allocators, and pool sender to every new connection handler, the server can pass a single `ServerContext` object.

The system contains a high-performance NFS server architecture where connections are lightweight but rely on heavy shared resources (disk I/O, memory pools). The `ServerContext` acts as the "root" of these shared resources. It ensures that when the server starts up, it initializes the `VfsPool` with the correct concurrency settings and wires it to the correct backend and memory allocator.

A typical usage scenario of the system involves the main server function (`handle_forever` in `lib.rs`). Before accepting connections, it constructs the `ServerContext`. Then, for every TCP connection accepted, it passes a reference to this context to the connection handler. The connection handler uses `get_vfs_pool` to dispatch work, `get_read_allocator` to allocate buffers for incoming requests, and `get_write_allocator` for responses.

Inside the system, the following things happen and they use this module:
1. **Resource Lifecycle Management**: The context owns the `VfsPool`. When the context is dropped (typically at server shutdown), the `VfsPool` is dropped, which in turn closes the worker channels and signals the workers to shut down. This provides a clean shutdown mechanism.
2. **Configuration Aggregation**: The context aggregates the configuration for the "read" and "write" paths. While the `VfsPool` specifically uses the `read_allocator` (as seen in the code), the context also holds the `write_allocator` to make it available to other parts of the connection stack (e.g., the write task) that need to allocate memory for sending replies.
3. **Type Erasure/Abstraction**: By using generic parameters `A`, `V`, and `B`, the context allows the upper layers of the server (like `lib.rs`) to choose the specific implementation of the filesystem and memory allocator, while the connection logic remains generic and unaware of these concrete types.

The critical aspect of this module is the **encapsulation of the server's "global state"**. It provides a stable, single point of access to the resources that define the server's behavior and capacity, ensuring that all connections operate against the same consistent environment.