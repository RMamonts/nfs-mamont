<!-- SPEC_HASH: ce55b804a829b3196fd5cb6b145590a102b63debb19fbc646968400dd7e2abf6 -->
# Module Specification

Module: nfs_mamont::task::global
Rust File: src/task/global/mod.rs

---

## 1. Dependencies

From the provided source code and module structure:

- **`crate::task::global::mount`**: This submodule is included to provide the implementation of the MOUNT protocol task. It is used to encapsulate the logic for handling mount requests that are global to the server instance.
- **`crate::task::global::nlm`**: This submodule is included to provide the implementation of the Network Lock Manager (NLM) protocol task. It is used to handle locking requests that must be managed centrally across all connections.
- **`crate::task::global::vfs`**: This submodule is included to provide the implementation of the Virtual File System (VFS) worker pool. It is used to manage the execution of filesystem operations (NFS procedures) using a pool of workers.

---

## 2. Mechanics

**Intent:**
The module serves as a **namespace aggregator** for global task implementations within the NFS server. Its purpose is to architecturally separate "global" tasks (which exist across all NFS client connections, such as MOUNT, NLM, and VFS) from connection-specific tasks. It provides a unified entry point for accessing the worker logic for these distinct protocols.

**Inputs:**
- None (This is a module declaration file).

**Outputs:**
- **Public Modules**: Re-exports the `mount`, `nlm`, and `vfs` modules, making their public interfaces (e.g., `MountTask`, `NlmTask`, `VfsPool`) available under the `nfs_mamont::task::global` path.

**Steps:**
1. **Declaration**: Declares `pub mod mount;`, making the MOUNT task logic available.
2. **Declaration**: Declares `pub mod nlm;`, making the NLM task logic available.
3. **Declaration**: Declares `pub mod vfs;`, making the VFS pool logic available.

**Edge Cases:**
- **Missing Implementation**: The module documentation lists "AUTH" as a global task with a "TODO: TBD" status. However, no corresponding `pub mod auth;` declaration exists in the code. This indicates an incomplete feature or a placeholder for future work.

**Complexity:**
- **Time**: O(1) (Compile-time organization).
- **Space**: O(1) (No runtime data structures).

**Determinism:**
- **Deterministic**. The module structure is fixed at compile time.

---

## 3. Dependency Mechanics

The current module acts as a container for the following key mechanisms provided by its submodules:

- **From `mount`**:
    - **Single-Task Dispatcher**: Provides a `MountTask` that serializes MOUNT protocol requests. This is crucial for ensuring that mount state modifications occur in a deterministic order.
- **From `nlm`**:
    - **Single-Task Dispatcher**: Provides an `NlmTask` that serializes Network Lock Manager requests. This centralizes lock state management to prevent race conditions.
- **From `vfs`**:
    - **Worker Pool Pattern**: Provides a `VfsPool` that manages a fixed number of `VfsTask` workers. This allows for parallel execution of filesystem operations (NFS READ/WRITE/etc.) while enforcing memory limits via an `Allocator`.

---

## 4. Data Model

**Entities:**
- None defined in this module.

**Relations:**
- None defined in this module.

**Global Invariants:**
- **Namespace Grouping**: All tasks defined within this module are conceptually "global" (shared across connections), as opposed to per-connection tasks.

---

## 5. Error Model

**Error Types:**
- None defined in this module.

**Error Propagation Strategy:**
- N/A.

**Recoverability:**
- N/A.

**Panics:**
- N/A.

---

## 6. Traits

The module does not define or implement any traits.

---

## 7. Overview

This module is used in order to **organize the architectural layout of the NFS server's background workers**, specifically grouping together those tasks that manage state or resources shared across all client connections (MOUNT, NLM, VFS). It solves the problem of code organization by establishing a clear boundary between "global" services and connection-specific logic.

This system contains a **collection of specialized asynchronous workers** that handle different aspects of the NFS protocol stack. By grouping them under `task::global`, the system signals that these components are instantiated once (e.g., during server startup) and their handles are shared or injected into connection handlers, rather than being created per connection.

A typical usage scenario of the system involves the main server initialization logic. The server will instantiate the `MountTask`, `NlmTask`, and `VfsPool` by accessing types defined within this module's submodules. These instances are then passed to the individual connection tasks (e.g., via `Arc` or channels) so that when a connection receives a request, it can dispatch it to the correct global worker.

Inside the system, the following things happen and they use this module:
1. **Service Discovery**: The server bootstrap process uses this module to locate and configure the core protocol handlers.
2. **Resource Isolation**: The module structure enforces the separation of concerns, ensuring that MOUNT protocol logic does not leak into NLM logic, and that VFS pooling is handled distinctly from the single-threaded dispatchers of MOUNT/NLM.

**Uncertainty:**
The module documentation explicitly mentions "AUTH (TODO: TBD)" as a global task. However, there is no implementation or module declaration for `auth` in the provided code. It is unclear if this refers to an RPC authentication mechanism (like `AUTH_SYS` or `RPCSEC_GSS`) or a different authorization layer, and its absence suggests it is either not yet implemented or handled elsewhere.