<!-- SPEC_HASH: ce55b804a829b3196fd5cb6b145590a102b63debb19fbc646968400dd7e2abf6 -->
# Module Specification

Module: nfs_mamont::task::global
Rust File: src/task/global/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **None**: The `*.facts.json` for this module lists no public structs, enums, traits, or functions, and the source code contains no `use` statements. This module acts purely as a namespace container (parent module) for its submodules and does not directly depend on external crates or other modules within the crate in its own definition file.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a **unified namespace** for "global" tasks within the NFS server architecture. This distinguishes long-running, stateful, or connection-independent background tasks (like MOUNT and NLM) and worker pools (VFS) from per-connection tasks.
- To **encapsulate protocol-specific concurrency strategies**. By grouping `mount`, `nlm`, and `vfs` here, the module signals that these components manage the server's core logic and state, separate from the network I/O layer.

Inputs:
- None. This module does not accept inputs; it defines the module tree structure.

Outputs:
- **`pub mod mount`**: Re-exports the `nfs_mamont::task::global::mount` module, which provides the `MountTask` actor.
- **`pub mod nlm`**: Re-exports the `nfs_mamont::task::global::nlm` module, which provides the `NlmTask` actor.
- **`pub mod vfs`**: Re-exports the `nfs_mamont::task::global::vfs` module, which provides the `VfsPool` worker pool.

Steps:
1. **Module Declaration**: The source code declares three public submodules: `mount`, `nlm`, and `vfs`.
2. **Documentation**: The module-level documentation describes the intent of managing tasks that exist across all NFS client connections, specifically listing MOUNT, NLM, and AUTH (the latter marked as TODO).

Edge Cases:
- **Incomplete Implementation**: The documentation mentions `AUTH` (Authentication) as a global task with a "TODO: TBD" status. However, there is no corresponding `pub mod auth;` in the provided source code, indicating this feature is planned but not yet implemented or exposed.

Complexity:
- **Time**: N/A (Compile-time organization).
- **Space**: N/A.

Determinism:
- **N/A**.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

Since this module aggregates submodules, the mechanics of the submodules constitute the public interface of this module.

- **From `nfs_mamont::task::global::mount`**:
 - **`MountTask`**: An actor task that serializes MOUNT protocol requests. It ensures that modifications to global mount state occur sequentially.
 - **`MountCommand`**: The message type used to communicate with the `MountTask`.

- **From `nfs_mamont::task::global::nlm`**:
 - **`NlmTask`**: An actor task that serializes Network Lock Manager (NLM) requests. It centralizes lock state management to prevent race conditions.
 - **`NlmCommand`**: The message type used to communicate with the `NlmTask`.

- **From `nfs_mamont::task::global::vfs`**:
 - **`VfsPool`**: A worker pool that manages a set of `VfsTask` instances. It provides concurrent execution of NFSv3 filesystem procedures, decoupling disk I/O from network handling.
 - **`VfsTask`**: The individual worker task that processes commands from the pool's queue.

---

## 4. Data Model

Entities:
- None defined in this file.

Relations:
- **Parent Module**: This module acts as the parent for `mount`, `nlm`, and `vfs`.

Global Invariants:
- **Namespace Isolation**: All tasks defined within this module are intended to be "global" in scope, meaning they are instantiated once per server instance rather than once per client connection.

## 5. Error Model

Error Types:
- None defined in this file.

Error Propagation Strategy:
- N/A.

Recoverability:
- N/A.

Panics:
- N/A.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **organize the server's backend task architecture** by grouping the subsystems responsible for handling different NFS protocols and operations. In the NFS-Mamont server, different protocols have different concurrency and state management requirements. For instance, the MOUNT protocol requires maintaining a list of exported directories and active clients, while the NLM protocol requires managing a lock table. Both require strict serialization to ensure consistency. In contrast, the core NFS (VFS) operations (like READ and WRITE) are largely stateless per-request and benefit from parallel execution.

The system contains a **hierarchical task structure** where this module serves as the root for "global" tasks. It exposes the `mount` and `nlm` modules, which implement the Actor pattern to serialize access to shared state, and the `vfs` module, which implements a Worker Pool pattern to maximize throughput for filesystem operations.

A typical usage scenario of the system involves the main server initialization logic.
1. The server initializes the `MountTask` from `nfs_mamont::task::global::mount` to handle mount requests.
2. It initializes the `NlmTask` from `nfs_mamont::task::global::nlm` to handle file locking.
3. It initializes the `VfsPool` from `nfs_mamont::task::global::vfs` with a specific number of workers to handle NFS procedure calls.
4. The server then distributes the senders (command channels) of these tasks to the individual connection tasks (which are not part of this module).

Inside the system, the following things happen and they use this module:
- **Architectural Separation**: This module enforces a clear boundary between "connection handling" (managing sockets, parsing bytes) and "protocol logic" (managing mounts, locks, and file data). By importing from `nfs_mamont::task::global`, the connection layer interacts with abstracted task handles rather than concrete implementations.
- **Concurrency Strategy Selection**: The module groups implementations that choose specific concurrency strategies based on the nature of the protocol. It tells the user that MOUNT and NLM are handled by single-threaded actors (safe for shared state), while VFS is handled by a thread pool (efficient for I/O bound tasks).

The critical aspect of this module is **semantic grouping**. It does not implement logic itself but provides a coherent interface to the server's core processing units, ensuring that the distinction between global state management and request processing is maintained at the module level.