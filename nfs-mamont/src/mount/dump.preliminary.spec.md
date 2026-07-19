<!-- SPEC_HASH: b370ba079c0208317879fb17b941eccd3c39932b2b85db6ffe3e964842ede39f -->
# Module Specification

Module: nfs_mamont::mount::dump
Rust File: src/mount/dump.rs

---

## 1. Dependencies

- **`super::MountEntry`** (from `nfs_mamont::mount`): Used to define the structure of individual records within the `Success` result. It encapsulates the relationship between a client hostname and a specific server directory path.
- **`trait_variant::make`**: A procedural macro attribute used to generate a `Send`-compatible version of the `Dump` trait. This allows the trait object to be safely transferred across thread boundaries, which is necessary for asynchronous RPC handlers that may run on different threads.

---

## 2. Mechanics

**Intent:**
To define the interface for the MOUNT protocol's Dump procedure (Procedure 2) as per RFC 1813. This interface allows a server to report the current list of remote filesystems that have been successfully mounted by clients.

**Inputs:**
- `&self`: A reference to the implementor of the trait (the server or service context).

**Outputs:**
- `Success`: A structure containing a vector of `MountEntry` records.

**Steps:**
1. The `dump` method is invoked asynchronously on the service implementation.
2. The implementation queries the internal state of the mount server to retrieve active mount records.
3. Each record is mapped to a `MountEntry` containing the client's hostname and the mounted directory path.
4. The collection of entries is wrapped in the `Success` struct and returned.

**Edge Cases:**
- **Empty State:** If no clients have currently mounted filesystems, the `mount_list` within `Success` will be an empty `Vec`.
- **Protocol Errors:** As per RFC 1813, this procedure does not return MOUNT protocol-specific errors. The interface reflects this by returning `Success` directly rather than a `Result`.

**Complexity:**
- **Time:** O(N), where N is the number of active mounts, as the list must be constructed and returned.
- **Space:** O(N), required to store the list of `MountEntry` structs in the `Success` response.

**Determinism:**
- Deterministic. Given the same internal server state, the method will always return the same list of mounts.

---

## 3. Dependency Mechanics

From `nfs_mamont::mount`:
- **`MountEntry`**: Acts as the data carrier for the dump operation. It combines a `HostName` and a `file::Path` to represent a single mount instance.
- **`HostName`**: Provides the mechanism for handling client identification, ensuring that hostnames are validated upon creation (via `new`) and accessible as string slices.

---

## 4. Data Model

**Entities:**
- **`Success`**: A wrapper struct representing the successful response of the Dump procedure.
- **`MountEntry`**: (Defined in parent module) Represents a tuple of `(hostname, directory)`.

**Relations:**
- `Success` (1) → `MountEntry` (0..N): A `Success` instance contains a list of zero or more mount entries.

**Global Invariants:**
- The `mount_list` in `Success` must accurately reflect the server's internal registry of clients that have successfully obtained file handles via the MNT procedure at the time of the call.

---

## 5. Error Model

**Error Types:**
- None defined in the public interface. The `dump` function signature is `async fn dump(&self) -> Success`.

**Error Propagation Strategy:**
- N/A. The interface explicitly forbids MOUNT protocol errors. Any underlying implementation errors (e.g., I/O errors reading state) must be handled internally or result in a panic/task failure, as they cannot be propagated via the return type defined by the RFC.

**Recoverability:**
- N/A.

**Panics:**
- **Allowed:** No (at the interface level).
- **Conditions:** The interface definition does not specify panic conditions. However, implementors may panic if internal invariants are violated (e.g., memory corruption), though this is not part of the observable contract.

---

## 6. Traits

- **`Dump`**: The primary trait defined by this module. It requires an asynchronous `dump` method.
- **`Send`**: The `Dump` trait is transformed into a `Send` trait object via `#[trait_variant::make(Send)]`.

---

## 7. Overview

This module is used in order to define the contract for the server-side "Dump" functionality of the NFS MOUNT protocol (version 3). It serves as a bridge between the abstract protocol requirements (RFC 1813) and the concrete data structures used by the `nfs_mamont` system.

This system contains the data definitions and service traits necessary to handle MOUNT protocol operations. Specifically, it focuses on the ability to enumerate active mounts.

A typical usage scenario of the system involves an NFS server receiving a DUMP request from a client or monitoring tool. The server invokes the `dump` method on an object implementing the `Dump` trait. The system retrieves the current state of mounted directories, formats them into a list of `MountEntry` (pairing client hostnames with directory paths), and returns them wrapped in a `Success` struct.

Inside the system, the following things happen and they use the `MountEntry` structure from the parent module to ensure consistency across different MOUNT procedures (like MNT and UMNT). The `Dump` trait abstracts the retrieval logic, allowing the underlying implementation to manage state (e.g., in-memory maps or persistent storage) while presenting a clean, asynchronous interface to the RPC layer. The use of `#[trait_variant::make(Send)]` ensures that this interface can be used in a multi-threaded asynchronous runtime (e.g., Tokio) where the task handling the request might be moved between threads.