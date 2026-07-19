<!-- SPEC_HASH: e35691374623732a34a5f87e7b07149215d1e2bc8444ce304df9c0d63cf4089f -->
# Module Specification

Module: nfs_mamont::service::mount::umntall
Rust File: src/service/mount/umntall.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::net::SocketAddr`**: Used to identify the client network endpoint. This serves as the key to locate and remove the specific set of mount entries associated with a client in the internal registry.
- **`crate::mount::umntall::Umntall`**: The trait defining the interface for the MOUNT v3 UMNTALL procedure. This module provides the concrete implementation of this trait for the `MountService` struct.
- **`super::MountService`**: The parent struct that holds the server's state. This implementation accesses `self.mounts` to mutate the runtime state of active mounts.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To execute the "Unmount All" logic by removing all mount records associated with a specific client address from the shared state.
- To ensure thread-safe access to the mount registry during the removal operation using asynchronous locking.

Inputs:
- `&self`: A reference to the `MountService` instance.
- `client_addr: SocketAddr`: The network address of the client whose mounts are to be removed.

Outputs:
- `()`: The function returns nothing. Success is implied by the completion of the function without panic.

Steps:
1. **Lock Acquisition**: The method calls `self.mounts.write().await` to acquire an exclusive write lock on the `RwLock` guarding the `MountRegistry`. This suspends the task until the lock is available, ensuring mutual exclusion with any other readers or writers of the mount table.
2. **Map Access**: Once the lock is acquired, it accesses the `by_client` field of the registry, which is a `HashMap<SocketAddr, HashSet<MountEntry>>`.
3. **Removal**: It calls the `remove` method on the `HashMap` with `client_addr` as the key. This deletes the entry if it exists, effectively dropping the associated `HashSet` of `MountEntry` records and freeing that memory.
4. **Lock Release**: The write guard is dropped at the end of the scope, automatically releasing the lock.

Edge Cases:
- **Client Not Found**: If the `client_addr` does not exist in the `by_client` map, the `remove` operation returns `None` and the function effectively does nothing (no-op).
- **Lock Contention**: If the mount registry is currently being read (e.g., by a `DUMP` request) or written to (e.g., by a `MNT` request), this operation will wait asynchronously until those operations complete.

Complexity:
- **Time**: 
  - Lock acquisition: Variable (depends on contention).
  - Hash map removal: O(1) average case for the map operation.
  - Memory deallocation: O(N) where N is the number of mounts the specific client had, as the `HashSet` and its contents must be dropped.
- **Space**: O(1) additional stack space.

Determinism:
- Deterministic (assuming the underlying `RwLock` and `HashMap` behave deterministically).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::service::mount`**:
 - **`MountService.mounts`**: This module relies on the `mounts` field being a `RwLock<MountRegistry>`. The implementation specifically depends on the locked data having a public or accessible field `by_client` which supports the `remove` method.
 - **`MountRegistry`**: The module relies on the structure of `MountRegistry` where `by_client` maps `SocketAddr` to a collection of mounts. The removal logic assumes that removing the key from this map clears all mounts for that client.

- **From `nfs_mamont::mount::umntall`**:
 - **`Umntall` Trait**: The module implements the `async fn umntall` signature defined here. It adheres to the contract that no MOUNT protocol errors are returned (return type is `()`).

---

## 4. Data Model

Entities:
- This module does not define new entities but operates on the `MountRegistry` defined in the parent module `nfs_mamont::service::mount`.

Relations:
- **`MountService` → `MountRegistry`**: The implementation mutates the `MountRegistry` state held by the service.

Global Invariants:
- After this method returns, the `by_client` map in the `MountRegistry` must not contain an entry for the provided `client_addr`.

## 5. Error Model

Error Types:
- None.

Error Propagation Strategy:
- Not applicable. The method signature does not allow returning errors. Any failure (e.g., lock poisoning) would result in a panic.

Recoverability:
- Not applicable.

Panics:
- **Allowed**: Yes (inherited from dependencies).
- **Conditions**: 
 - If the `RwLock` on `self.mounts` is poisoned (e.g., a previous panic occurred while holding the lock), calling `.write().await` will panic.

---

## 6. Traits

List which external traits this module implements:
- **`crate::mount::umntall::Umntall`**: Implemented for `super::MountService`.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **provide the concrete implementation for the cleanup of client state within the MOUNT v3 protocol service**. It connects the abstract definition of the "Unmount All" procedure (defined in `nfs_mamont::mount::umntall`) with the concrete state management logic of the server (defined in `nfs_mamont::service::mount`).

The system contains a complex implementation of an NFS server where the `MountService` tracks active filesystem mounts in memory. This tracking is necessary for the server to know which clients are using which paths. However, when a client disconnects or explicitly requests to unmount all paths, this state must be purged to prevent memory leaks and to ensure that if the client reconnects, it is treated as a new session.

A typical usage scenario of the system involves a client sending a `UMNTALL` RPC request. The RPC dispatcher, which holds a reference to the `MountService`, invokes the `umntall` method. This module's implementation then executes the logic to find the client's entry in the shared registry and delete it.

Inside the system, the following things happen and they use this module:
1. **State Synchronization**: The RPC handler calls this method. The method acquires a write lock on the `mounts` registry. This ensures that while the cleanup is happening, no other request can read or modify the mount list, guaranteeing consistency.
2. **Resource Deallocation**: By calling `remove` on the `by_client` map, the module triggers the dropping of the `HashSet` containing all `MountEntry` records for that client. This frees the memory associated with those records.
3. **Protocol Compliance**: The implementation ensures that the server adheres to the MOUNT v3 specification (RFC 1813) regarding the `UMNTALL` procedure, which requires the server to discard all mount information for the requesting client.

Without this module, the `MountService` would have no way to process `UMNTALL` requests, leading to a state where the server indefinitely retains mount records for clients that have already disconnected, eventually exhausting memory.