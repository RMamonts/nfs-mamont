<!-- SPEC_HASH: 775772341bd833558d7a3d6b83fe3963fb4d171d042c99832e6440d3546ad739 -->
# Module Specification

Module: nfs_mamont::service::mount::umnt
Rust File: src/service/mount/umnt.rs

---

## 1. Dependencies

- **`std::net::SocketAddr`**
    - **Purpose:** Used as the primary key to identify the client within the internal mount table (`by_client`). This allows the service to distinguish between different clients that may have mounted the same directory.

- **`crate::mount::umnt::{Args, Umnt}`**
    - **Purpose:** Provides the trait `Umnt` which this module implements for `MountService`. It also provides the `Args` struct, which encapsulates the `dirpath` (the directory to be unmounted) that the client wishes to release.

- **`super::MountService`**
    - **Purpose:** The concrete service struct that holds the server's state. This module adds the unmount behavior to it. The implementation relies on `MountService` containing a field named `mounts` which manages the state.

---

## 2. Mechanics

**Intent:**
To modify the server's internal mount state by removing a specific directory path from the list of active mounts associated with a specific client IP address. This implements the server-side logic for the MOUNT v3 UMNT procedure.

**Inputs:**
- `args: Args`: Contains the `dirpath` (type `file::Path`) identifying the directory to unmount.
- `client_addr: SocketAddr`: The network address of the client requesting the unmount.

**Outputs:**
- `()`: The function returns no value. Success is indicated by the absence of a panic or error.

**Steps:**
1. Acquire a write lock on `self.mounts` (implied to be an asynchronous lockable structure like `RwLock`).
2. Access the `by_client` map within the mounts data.
3. Attempt to retrieve the mutable list of mount entries associated with `client_addr`.
4. If the list exists:
    a. Iterate over the entries and retain only those where the `entry.directory` does **not** match `args.dirpath`.
    b. Check if the resulting list of entries is empty.
    c. If empty, remove the `client_addr` key from the `by_client` map entirely to free resources.
5. If the client address is not found in the map, perform no action (no-op).

**Edge Cases:**
- **Client not in table:** If `client_addr` is not a key in `by_client`, the operation does nothing.
- **Path not mounted by client:** If the client exists but `args.dirpath` is not in its list, `retain` leaves the list unchanged.
- **Concurrent access:** The use of `.write().await` implies that concurrent modifications to the mount table are serialized, preventing race conditions.

**Complexity:**
- **Time:** O(N), where N is the number of directories currently mounted by the specific client (due to the linear scan performed by `retain`).
- **Space:** O(1) auxiliary space. The operation modifies the vector in-place.

**Determinism:**
- **Deterministic**

---

## 3. Dependency Mechanics

- **`nfs_mamont::mount::umnt::Umnt`**
    - **Idempotency Requirement:** The dependency specification notes that the procedure must be idempotent and return no errors. This module adheres to this by using `retain`, which safely handles cases where the entry is already missing, and by returning `()`.

- **`nfs_mamont::mount::MountEntry`**
    - **Field Access:** The code accesses `entry.directory`. This relies on the `MountEntry` struct (defined in the `mount` module) having a public or accessible `directory` field of a type comparable to `file::Path` (specifically `args.dirpath`).

---

## 4. Data Model

**Entities:**
- **`MountService`**
    - The stateful container for the MOUNT protocol service.
    - *Assumption:* Contains a field `mounts`. Based on usage (`self.mounts.write().await` and `mounts.by_client`), this is inferred to be a smart pointer (e.g., `Arc<RwLock<...>>`) to a structure containing a map `by_client: HashMap<SocketAddr, Vec<MountEntry>>`.

- **`Mounts` (Internal Inferred Structure)**
    - Holds the active mount state.
    - Contains `by_client`: A map where keys are client addresses and values are lists of `MountEntry`.

**Relations:**
- `MountService` owns `Mounts`.
- `Mounts` aggregates `MountEntry` by `SocketAddr`.

**Global Invariants:**
- **Empty List Cleanup:** The `by_client` map will not contain keys that map to empty vectors. The logic explicitly removes the key if the vector becomes empty after filtering.

---

## 5. Error Model

**Error Types:**
- None.

**Error Propagation Strategy:**
- **Silent Success:** The implementation strictly follows the interface contract which does not allow returning errors. Invalid requests (e.g., unmounting a non-existent path) are silently ignored.

**Recoverability:**
- N/A.

**Panics:**
- **Allowed:** Yes.
- **Conditions:**
    - If the lock `self.mounts` is poisoned (e.g., a previous thread panicked while holding the lock), the call to `.write().await` may panic.
    - If `MountService` is initialized incorrectly (e.g., `mounts` field missing), though this is prevented by the compiler.

---

## 6. Traits

- **`crate::mount::umnt::Umnt`**: Implemented for `MountService`.

---

## 7. Overview

This module is used in order to maintain the accuracy of the server's internal view of active NFS mounts by processing client requests to unmount directories. It acts as the concrete logic layer that updates the shared state managed by `MountService` in response to the `UMNT` RPC procedure.

The system containing this module is an NFS server implementing the MOUNT protocol. The MOUNT protocol is responsible for managing the relationship between client IPs and server directory paths. While the `nfs_mamont::mount::umnt` module defines the *contract* (the "what") for unmounting, this module defines the *implementation* (the "how") regarding state mutation.

A typical usage scenario involves a client shutting down or a user executing a `umount` command. The client sends a `UMNT` request specifying the directory path. The RPC handler dispatches this to `MountService`. This module then acquires exclusive access to the mount table, locates the specific client's record, and filters out the specified directory. If this was the last directory the client had mounted, the module performs a cleanup by removing the client's entry entirely, ensuring the server does not waste memory tracking clients with no active mounts.

Inside the system, the following things happen: The asynchronous runtime handles the request, eventually reaching the `umnt` method. The method waits for a write lock on the shared state, ensuring consistency. It then performs a precise modification of the in-memory data structure (`HashMap` of `Vec`s) to reflect the client's intent. This ensures that subsequent administrative queries (like `MOUNTPROC_DUMP`) will not show the unmounted directory for that client. The module relies on the assumption that `self.mounts` is a lockable structure containing a `by_client` map, as specific type definitions for `MountService`'s fields were not provided in the context but are inferred from method calls.