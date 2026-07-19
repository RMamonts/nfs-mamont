<!-- SPEC_HASH: e35691374623732a34a5f87e7b07149215d1e2bc8444ce304df9c0d63cf4089f -->
# Module Specification

Module: nfs_mamont::service::mount::umntall
Rust File: src/service/mount/umntall.rs

---

## 1. Dependencies

From the source code analysis, the following external dependencies are utilized:

- **`std::net::SocketAddr`**: Used as the key identifier for the client whose mount records are to be removed. It is passed as an argument to the implementation.
- **`crate::mount::umntall::Umntall`**: The trait defining the interface for the MOUNT v3 UMNTALL procedure. This module provides the concrete implementation of this trait for the `MountService` struct.
- **`super::MountService`**: The struct for which the `Umntall` trait is implemented. This struct is assumed to hold the runtime state of mount information.

---

## 2. Mechanics

This module provides the concrete service-layer implementation for the NFS MOUNT protocol "UMNTALL" procedure. It bridges the abstract protocol definition with the internal state management of the `MountService`.

Intent:
- To remove all mount records associated with a specific client IP address from the service's internal state upon request.

Inputs:
- `&self`: A reference to the `MountService` instance.
- `client_addr: SocketAddr`: The network address of the client requesting the unmount of all filesystems.

Outputs:
- `()`: The function returns nothing. Success is implied by the completion of the function without panicking.

Steps:
1. The method is called with a specific `client_addr`.
2. It accesses `self.mounts`, which is inferred to be a shared, lockable state container (likely a `RwLock` or `Mutex` given the `.write().await` syntax).
3. It acquires a write lock on this state asynchronously.
4. Within the locked state, it accesses a collection or map field named `by_client`.
5. It calls `remove(&client_addr)` on this collection, deleting all entries associated with the key.

Edge Cases:
- **Client not found**: If the `client_addr` does not exist in the `by_client` collection, the `remove` operation is a no-op (standard behavior for collection removal in Rust).
- **Lock contention**: If the write lock on `self.mounts` is held by another task, the current task will yield (await) until the lock is released.

Complexity:
- Time: O(1) average time complexity for the hash map removal operation, plus the time required to acquire the write lock.
- Space: O(1) additional space used.

Determinism:
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::mount::umntall`**:
    - **Trait Contract**: The `Umntall` trait defines the `async fn umntall(&self, client_addr: SocketAddr)` signature. This module fulfills this contract, ensuring that the `MountService` can be used wherever the `Umntall` trait is required (e.g., in a generic RPC dispatcher).
    - **Protocol Semantics**: The dependency spec notes that "AUTH_UNIX authentication or better is required" and "There are no MOUNT protocol errors which can be returned". This implementation adheres to this by not returning a `Result` and assuming authentication has been handled prior to invocation.

- **From `nfs_mamont::service::mount`**:
    - **Service Structure**: The `MountService` struct acts as the implementor. While the specific fields are not listed in the provided JSON facts for the dependency, the code in this module implies that `MountService` contains a field `mounts` capable of being locked and written to asynchronously.

---

## 4. Data Model

Entities:
- **`MountService`**: The service struct holding the state. *Assumption*: It contains a field `mounts` which is a synchronization primitive (e.g., `tokio::sync::RwLock`) guarding the internal mount table.
- **`SocketAddr`**: The key used to identify a client.
- **`by_client`**: *Assumption*: A field within the guarded `mounts` data structure, likely a `HashMap` or similar mapping, where keys are `SocketAddr` and values represent the list of mounted paths for that client.

Relations:
- `MountService` contains `mounts`.
- `mounts` contains `by_client`.
- `by_client` maps `SocketAddr` to mount entries.

Global Invariants:
- Upon successful return of `umntall`, the `by_client` mapping within `MountService` must not contain any entries for the provided `client_addr`.

---

## 5. Error Model

Error Types:
- None. The return type is `()`.

Error Propagation Strategy:
- Not applicable. The method does not return a `Result`. Internal errors (like lock poisoning) will result in a panic rather than a returned error.

Recoverability:
- Not applicable via the return type.

Panics:
- Allowed: Yes.
- Conditions: The method may panic if the lock `self.mounts` is poisoned (i.e., a previous thread holding the lock panicked). This is standard behavior for Rust mutexes/rwlocks.

---

## 6. Traits

List which external traits this module implements:
- **`crate::mount::umntall::Umntall`**: Implemented for `MountService`.

---

## 7. Overview

This module is used in order to implement the logic required to purge all mount records for a specific client from the server's memory, effectively handling the "Unmount All" scenario of the NFS MOUNT protocol.

The system incorporating this module is an NFSv3 server implementation. The MOUNT protocol is responsible for establishing the relationship between a client and a server's filesystem. When a client is finished or shutting down, it may send a `UMNTALL` request to tell the server to forget all previous mounts made by that client.

A typical usage scenario of the system involves a client sending an RPC request for the `UMNTALL` procedure. The server's RPC layer, which is generic over the `Umntall` trait, receives this request and extracts the client's address. It then invokes the `umntall` method on the `MountService`.

Inside the system the following things happen and they use this module:
1. The generic RPC handler calls `mount_service.umntall(client_addr)`.
2. The implementation in this module acquires exclusive access to the mount table (`self.mounts.write().await`).
3. It removes the client's entry from the `by_client` index.
4. The lock is released, and the RPC layer sends a void response to the client.

This module is necessary to decouple the generic protocol handling (defined in `nfs_mamont::mount::umntall`) from the specific state management logic of the `MountService`. It ensures that the unmount operation is thread-safe (via the write lock) and non-blocking (via `await`), allowing the server to handle high concurrency. The explicit removal of the client from the `by_client` map ensures that subsequent administrative queries or mount checks do not consider this client as active.