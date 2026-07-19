<!-- SPEC_HASH: 2b7d1fbdf92c09d62d37914d4084826cce22d6969513703b11d62cab82577bad -->
# Module Specification

Module: nfs_mamont::service::mount::dump
Rust File: src/service/mount/dump.rs

---

## 1. Dependencies

- **`crate::mount::dump`**: Used to import the `Dump` trait and the `Success` struct. The `Dump` trait defines the asynchronous interface required by the MOUNT v3 protocol, and `Success` is the specific return type wrapping the list of active mounts.
- **`super::MountService`**: The concrete service struct for which the `Dump` trait is implemented. This struct holds the internal state (`mounts`) representing the current active mount sessions.

---

## 2. Mechanics

**Intent:**
To provide the concrete implementation of the MOUNT v3 `DUMP` procedure for the `MountService`. This involves retrieving the internal registry of mounted filesystems, transforming the data structure from an internal representation (grouped by client) to the protocol-defined flat list, and returning it.

**Inputs:**
- `&self`: A reference to the `MountService` instance.

**Outputs:**
- `Success`: A struct containing a `mount_list` (`Vec<MountEntry>`).

**Steps:**
1. Acquire a read lock on `self.mounts` asynchronously using `.read().await`. This ensures thread-safe access to the shared state.
2. Access the `by_client` field of the locked data structure.
3. Iterate over the values of `by_client` (which are assumed to be collections of mount entries).
4. Use `flat_map` to flatten these collections into a single iterator of mount entries.
5. Clone each entry (`.cloned()`) to disassociate them from the lock's lifetime.
6. Collect the results into a `Vec`.
7. Wrap the vector in the `Success` struct and return it.

**Edge Cases:**
- **Empty Registry:** If `self.mounts` contains no entries, the method returns `Success` with an empty `mount_list`.
- **Lock Contention:** The method will wait asynchronously if the write lock is held by another operation (e.g., a client mounting or unmounting).

**Complexity:**
- **Time:** O(N), where N is the total number of active mount entries across all clients. This accounts for the iteration, cloning, and collection.
- **Space:** O(N), required to allocate the new `Vec<MountEntry>` in the `Success` struct.

**Determinism:**
- Deterministic. The output is strictly determined by the state of `self.mounts` at the moment the read lock is acquired.

---

## 3. Dependency Mechanics

From `nfs_mamont::mount::dump`:
- **`Dump` Trait**: Defines the contract `async fn dump(&self) -> Success`. This module provides the implementation logic for this contract.
- **`Success` Struct**: Acts as the container for the result. The module constructs this struct by populating its `mount_list` field.

From `nfs_mamont::mount`:
- **`MountEntry`**: The data element being cloned and collected. The implementation relies on `MountEntry` implementing `Clone` (implied by `.cloned()`).

---

## 4. Data Model

**Entities:**
- **`MountService`**: The service context containing the internal state.
- **`Internal State` (inferred)**: Accessed via `self.mounts`. It is inferred to be a lockable structure containing a field `by_client`, which is likely a map where keys are client identifiers and values are collections of `MountEntry`.
- **`Success`**: The output wrapper containing a vector of `MountEntry`.

**Relations:**
- `Internal State` (1) → `MountEntry` (0..N): The internal state aggregates multiple mount entries.
- `Success` (1) → `MountEntry` (0..N): The output flattens the internal structure into a simple list.

**Global Invariants:**
- The returned `mount_list` must accurately reflect the state of the server at the time of the lock acquisition.
- The order of entries in the `mount_list` depends on the iteration order of the internal `by_client` map and its values (which is not explicitly sorted in the code).

---

## 5. Error Model

**Error Types:**
- None defined in the signature. The function returns `Success` directly, not a `Result`.

**Error Propagation Strategy:**
- N/A. The MOUNT v3 DUMP procedure specification (RFC 1813) does not define protocol errors for this procedure. Consequently, the implementation does not return errors.

**Recoverability:**
- N/A.

**Panics:**
- **Allowed:** No (at the interface level).
- **Conditions:** The code assumes `self.mounts` is a valid lockable object. If the internal lock is poisoned (a rare state indicating a panic occurred while holding the write lock), the behavior depends on the specific `RwLock` implementation used (e.g., `tokio::sync::RwLock` may panic on access if poisoned, though standard behavior varies). The code does not explicitly handle poisoning.

---

## 6. Traits

- **`crate::mount::dump::Dump`**: Implemented for `MountService`.

---

## 7. Overview

This module is used in order to bridge the abstract protocol definition of the MOUNT DUMP operation with the concrete internal state management of the NFS server. It transforms the server's internal view of active mounts (likely organized for efficient lookup by client) into the flat list required by the NFS protocol.

This system contains the service layer logic for the MOUNT protocol. It relies on the `MountService` to maintain the authoritative state of which clients have mounted which directories.

A typical usage scenario of the system involves an NFS client or administrator invoking the DUMP procedure. The RPC layer dispatches the call to the `dump` method on the `MountService`. This implementation acquires a read lock on the internal registry to ensure a consistent snapshot. It then iterates over the internal data structure—specifically, the `by_client` mapping—and flattens the collections of mount entries into a single vector. By cloning the entries, it ensures the returned data is valid even after the lock is released.

Inside the system, the following things happen and they use the `flat_map` and `cloned` iterators to efficiently transform the data structure without mutating the original state. The module assumes that `self.mounts` is an asynchronous read-write lock (like `tokio::sync::RwLock`) protecting a structure containing a `by_client` field, though the exact definition of `MountService`'s fields is not visible in the public interface facts provided. This abstraction allows the internal storage format to change (e.g., from a `HashMap` to a `BTreeMap`) without affecting the protocol interface.