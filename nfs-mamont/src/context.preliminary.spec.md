<!-- SPEC_HASH: 82c53027039b9101eb38a6495af80d5edf87c927558590bf70702e3f9b730b31 -->
# Module Specification

Module: nfs_mamont::context
Rust File: src/context.rs

---

## 1. Dependencies

From the provided context and dependency facts:

- **`std::sync::Arc`**: Used to enable shared ownership of the backend and allocators across multiple threads/tasks. This allows the `ServerContext` to hand out references to these resources without transferring ownership, ensuring they remain alive as long as the server context exists.
- **`std::num::NonZeroUsize`**: Used to specify the size of the VFS worker pool, guaranteeing at compile time that the pool size is greater than zero.
- **`crate::allocator::{Allocator, Buffer}`**: Defines the memory management interface. The module holds two instances of the allocator (`read` and `write`) to manage buffers for network I/O, enforcing the server's memory strategy.
- **`crate::task::global::vfs::VfsPool`**: The worker pool implementation responsible for executing filesystem operations asynchronously. The `ServerContext` initializes this pool, injecting the backend and read allocator into it.
- **`crate::vfs::Vfs`**: The trait defining the filesystem operations (e.g., read, write, lookup). The module holds a reference-counted pointer to the concrete implementation (`backend`) that performs the actual file system logic.

---

## 2. Mechanics

**Intent:**
The module acts as a central dependency injection container for the NFS server. Its purpose is to aggregate the shared, long-lived resources—the filesystem backend, memory allocators, and the VFS worker pool—into a single context object. This object is constructed once at startup and passed to connection handlers, providing them with access to the necessary infrastructure to process NFS requests.

**Inputs:**
- **`backend: Arc<V>`**: A shared reference to the filesystem implementation.
- **`read_allocator: Arc<A>`**: A shared reference to the allocator used for read buffers.
- **`write_allocator: Arc<A>`**: A shared reference to the allocator used for write buffers.
- **`vfs_pool_size: NonZeroUsize`**: The number of worker tasks to spawn in the VFS pool.

**Outputs:**
- **`ServerContext<A, V, B>`**: An instance containing the initialized `VfsPool` and references to the shared resources.

**Steps:**
1. **Initialization (`new`)**:
   - Accepts the `backend`, `read_allocator`, and `write_allocator` wrapped in `Arc`.
   - Invokes `VfsPool::new`, passing the `vfs_pool_size`, a clone of the `backend`, and a clone of the `read_allocator`. This initializes the worker pool and its internal command channel.
   - Constructs the `ServerContext` struct, storing the newly created `vfs_pool` and the original `Arc` references to the allocators and backend.
2. **Accessors (`get_*` methods)**:
   - `get_vfs_pool`: Returns a reference to the internal `VfsPool`.
   - `get_backend`: Clones the `Arc<V>` and returns it, incrementing the reference count.
   - `get_read_allocator`: Clones the `Arc<A>` for the read allocator and returns it.
   - `get_write_allocator`: Clones the `Arc<A>` for the write allocator and returns it.

**Edge Cases:**
- None explicitly handled in this module. The logic relies on the validity of the inputs (e.g., `vfs_pool_size` must be valid).

**Complexity:**
- **Time**: O(1) for construction (mostly pointer cloning) and O(1) for accessors.
- **Space**: O(1) additional space for the struct itself (it holds pointers/handles).

**Determinism:**
- Deterministic. The construction and access operations are direct memory operations without branching logic dependent on external state.

---

## 3. Dependency Mechanics

- **`VfsPool` (from `nfs_mamont::task::global::vfs`)**:
  - **Initialization**: The `ServerContext` is responsible for the lifecycle initiation of the `VfsPool`. It passes the `read_allocator` to the pool so that the workers can allocate memory for file data being read from the disk. It also passes the `backend` so workers can execute filesystem logic.
  - **Dispatching**: By exposing `get_vfs_pool`, the context allows the network layer to obtain the `Sender` side of the pool's channel to enqueue work.

- **`Allocator` (from `nfs_mamont::allocator`)**:
  - **Separation of Concerns**: The context maintains two distinct allocator instances (`read` and `write`). This suggests that the system distinguishes between memory used for incoming data (reads from disk/network) and outgoing data, potentially allowing for different tuning or accounting strategies, though the type `A` is currently the same for both.

---

## 4. Data Model

**Entities:**
- **`ServerContext<A, V, B>`**: The main container struct.
  - `vfs_pool`: The worker pool for NFS operations.
  - `read_allocator`: Shared handle to the memory allocator for reads.
  - `write_allocator`: Shared handle to the memory allocator for writes.
  - `backend`: Shared handle to the filesystem implementation.

**Relations:**
- **`ServerContext` → `VfsPool` (1:1)**: The context owns the pool instance.
- **`ServerContext` → `Allocator` (1:2)**: The context holds shared references to two allocator instances.
- **`ServerContext` → `Vfs` (1:1)**: The context holds a shared reference to the backend.

**Global Invariants:**
- The `vfs_pool` contained within the context is initialized with the specific `backend` and `read_allocator` also held by the context.
- The `Arc` counters for `backend` and allocators are incremented when the context is created and again whenever `get_backend` or `get_*_allocator` are called.

---

## 5. Error Model

**Error Types:**
- None defined or generated by this module.

**Error Propagation Strategy:**
- N/A (This module performs no I/O or logic that results in errors).

**Recoverability:**
- N/A.

**Panics:**
- **Allowed**: No (No code in this module triggers panics).

---

## 6. Traits

The module defines and implements no external traits. It acts as a consumer of the `Allocator`, `Buffer`, and `Vfs` traits via generic constraints.

---

## 7. Overview

This module is used in order to **centralize the shared runtime resources of the NFS server**, facilitating a clean architecture where the main server loop (or connection handlers) can access all necessary dependencies (filesystem, memory, workers) via a single reference.

This system contains a **dependency aggregation pattern** that decouples the configuration of the server's infrastructure from its execution logic. By grouping the `Vfs` backend, memory allocators, and the `VfsPool` into one struct, the system ensures that these components are initialized in the correct order and with the correct dependencies (e.g., the pool receives the allocator) before the server starts accepting traffic.

A typical usage scenario of the system involves the application startup sequence. The main function configures the memory allocators and the filesystem backend. It then instantiates `ServerContext::new`, passing these components along with the desired worker pool size. The resulting `ServerContext` is passed to `handle_forever` (from `lib.rs`). As new connections are accepted, the connection handlers use the context to retrieve the `VfsPool` (to dispatch file operations) and the allocators (to manage buffers for network transmission).

Inside the system, the following things happen and they use this module:
- **Resource Sharing**: The `ServerContext` uses `Arc` to allow thousands of concurrent connections to access the same filesystem backend and memory pools without expensive copying or synchronization overhead beyond what `Arc` provides.
- **Lifecycle Management**: The context owns the `VfsPool`. When the `ServerContext` is dropped, the `VfsPool` is also dropped, which triggers the shutdown of the worker tasks and the closing of the command channel, ensuring a graceful shutdown of the processing layer.
- **Configuration Injection**: The module explicitly separates `read_allocator` and `write_allocator`. While the `VfsPool` only uses the `read_allocator` (as seen in its initialization), the context exposes the `write_allocator` via `get_write_allocator`. This implies that other components of the system (likely the network writer tasks) depend on the `ServerContext` to access the specific memory pool designated for write operations.