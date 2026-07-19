<!-- SPEC_HASH: 775772341bd833558d7a3d6b83fe3963fb4d171d042c99832e6440d3546ad739 -->
# Module Specification

Module: nfs_mamont::service::mount::umnt
Rust File: src/service/mount/umnt.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::net::SocketAddr`**: Used to uniquely identify the client network endpoint. This serves as the key to locate the specific set of mount entries belonging to the client requesting the unmount.
- **`crate::mount::umnt::{Args, Umnt}`**: Used to define the contract for the unmount operation. `Args` provides the `dirpath` to be removed, and the `Umnt` trait is implemented here to provide the concrete logic for the `MountService`.
- **`super::MountService`**: The parent struct for which the `Umnt` trait is implemented. It provides access to the `mounts` field, which holds the runtime state of active mounts.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement the server-side logic for the MOUNT v3 `UMNT` procedure, specifically mutating the internal mount registry to reflect that a client no longer requires access to a specific directory.

Inputs:
- `args: Args`: Contains the `dirpath` (`file::Path`) that the client wishes to unmount.
- `client_addr: SocketAddr`: The network address of the client issuing the request.

Outputs:
- `()`: The function returns nothing. Success is implied by the completion of the asynchronous execution without panic.

Steps:
1. **Lock Acquisition**: The method acquires a write lock on `self.mounts` using `write().await`. This ensures exclusive access to the mount registry for the duration of the operation, preventing race conditions with other mount/unmount requests.
2. **Client Lookup**: It attempts to retrieve a mutable reference to the set of mount entries associated with the `client_addr` via `mounts.by_client.get_mut(&client_addr)`.
3. **Entry Removal**: If the client exists in the registry, the method calls `retain` on the set of entries. The closure `|entry| entry.directory != args.dirpath` filters the set, keeping only entries where the directory does *not* match the requested `dirpath`. This effectively removes the specific mount entry.
4. **Cleanup**: After filtering, the method checks if the set of entries for the client is now empty (`entries.is_empty()`). If so, it removes the `client_addr` key entirely from the `by_client` map to free resources.

Edge Cases:
- **Client Not Found**: If `get_mut` returns `None` (the client has no active mounts), the method does nothing.
- **Path Not Found**: If the client exists but the specific `dirpath` is not in their mount list, the `retain` predicate retains all entries, and the state remains unchanged.
- **Concurrent Requests**: The `write().await` ensures that if multiple `UMNT` requests arrive for the same client, they are serialized.

Complexity:
- **Time**: O(N), where N is the number of directories currently mounted by the specific client. This is due to the `retain` operation iterating over the client's mount set.
- **Space**: O(1) auxiliary space.

Determinism:
- Deterministic. The final state depends solely on the initial state and the input arguments.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::mount::umnt`**:
 - **`Args`**: Provides the `dirpath` field which is used as the filter criterion in the `retain` closure.
 - **`Umnt` Trait**: Defines the asynchronous signature `async fn umnt(&self, args: Args, client_addr: SocketAddr)` which this module implements.

- **From `nfs_mamont::service::mount`**:
 - **`MountService`**: Acts as the container for the state. This module relies on the existence of the `mounts` field, which is a `RwLock<MountRegistry>`.
 - **`MountRegistry`**: Provides the `by_client` field (`HashMap<SocketAddr, HashSet<MountEntry>>`). This module assumes this structure exists to perform the lookup and mutation.

- **From `nfs_mamont::mount`**:
 - **`MountEntry`**: This module relies on the `MountEntry` struct having a public `directory` field of type `file::Path` to perform the comparison `entry.directory != args.dirpath`.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on the state defined in `super::MountService`.

Relations:
- **`MountService` implements `Umnt`**: This module provides the implementation logic linking the service's state to the protocol interface.

Global Invariants:
- **Consistency**: The `by_client` map must accurately reflect the active mounts. This module ensures that if a `UMNT` call succeeds, the specific directory is no longer associated with the client in the map.
- **Lock Safety**: The `mounts` field must only be accessed via the `RwLock` guard obtained by `write().await` to ensure thread safety.

## 5. Error Model

Error Types:
- None defined in this module.

Error Propagation Strategy:
- **Silent Success**: The function returns `()`. It does not return a `Result`. Errors such as "client not found" or "path not mounted" are treated as no-ops rather than errors, consistent with the idempotent nature of the `UMNT` procedure defined in RFC 1813.

Recoverability:
- **Idempotent**: The operation is safe to retry. If the entry is already gone, the state remains valid.

Panics:
- **Allowed**: No explicit panics.
- **Conditions**: A panic might occur if the `RwLock` is poisoned (e.g., a previous holder of the write lock panicked), but this is a runtime failure of the lock primitive, not explicit logic in this module.

---

## 6. Traits

List which external traits this module implements:
- **`crate::mount::umnt::Umnt`**: Implemented for `super::MountService`.

List which traits this module defines:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **enforce the lifecycle management of client mounts within the MOUNT protocol service**. It serves as the concrete handler that translates a client's explicit request to unmount a directory into a mutation of the server's shared state.

The system contains a complex architecture where the `MountService` acts as the central authority for tracking which clients are accessing which exports. The `Umnt` trait, defined in a lower-level module (`nfs_mamont::mount::umnt`), specifies the *interface* for unmounting (what arguments are needed, that it is async), but it does not know *how* the data is stored. This module bridges that gap. It knows that `MountService` stores data in a `HashMap` keyed by `SocketAddr` and contains `MountEntry` objects with `directory` fields.

A typical usage scenario of the system involves a client sending an RPC `UMNT` request for the path "/export/data". The RPC dispatcher calls the `umnt` method implemented here. The module then acquires a lock on the global mount registry to ensure no other thread is modifying the state simultaneously. It looks up the client's IP address, filters out the entry for "/export/data", and cleans up the client record if they have no other mounts left.

Inside the system, the following things happen and they use this module:
1. **State Synchronization**: When the `MountService` is created, it is passed to the RPC handler. This module provides the specific logic required to handle the `UMNT` procedure, ensuring that the `MountService`'s internal `by_client` map is updated correctly.
2. **Resource Reclamation**: By removing entries from the `HashSet` and potentially removing the client key from the `HashMap`, this module prevents memory leaks caused by stale mount entries accumulating over time as clients disconnect or unmount filesystems.
3. **Protocol Compliance**: The implementation ensures that the server adheres to the MOUNT v3 specification (RFC 1813) by performing the unmount silently (no errors returned for non-existent paths) and operating atomically on the client's set of mounts.

Without this module, the `MountService` would not satisfy the `Umnt` trait, meaning the server could accept mount requests but would have no mechanism to process unmount requests, leading to an ever-growing mount table and incorrect behavior for clients attempting to manage their connections.