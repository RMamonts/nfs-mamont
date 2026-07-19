<!-- SPEC_HASH: 7d383c7055711953e2ec478be0b894249083345f14b5ee7883f4fc4b1b62169d -->
# Module Specification

Module: nfs_mamont::vfs::commit
Rust File: src/vfs/commit.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import `WccData` and `Error`. `WccData` is required in both `Success` and `Fail` structs to provide Weak Cache Consistency information to the client. `Error` is used in the `Fail` struct to indicate the specific reason for the operation failure.
- **`super::file`**: Used to import `Handle`. The `Handle` is used in the `Args` struct to identify the target file system object for the commit operation.
- **`vfs::write`**: Used to import `Verifier`. The `Verifier` is included in the `Success` struct to allow the client to validate that the server has not rebooted between a previous `WRITE` operation and this `COMMIT` operation.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface contract for the NFSv3 `COMMIT` procedure.
- To specify the arguments required to flush a range of file data to stable storage.
- To define the response structures, including the `Verifier` used for server state validation and `WccData` for cache consistency.
- To ensure that the implementation can handle partial range flushes (offset + count) or full flushes (count = 0).

Inputs:
- **`Args`**: A structure containing:
 - `file`: The `file::Handle` identifying the file.
 - `offset`: A `u64` indicating the starting byte position for the flush.
 - `count`: A `u32` indicating the number of bytes to flush. If `0`, the flush extends to the end of the file.

Outputs:
- **`Result<Success, Fail>`**:
 - **`Success`**: Contains `file_wcc` (attributes before/after) and `verifier` (a cookie to detect server reboots).
 - **`Fail`**: Contains `error` (the specific failure reason) and `file_wcc` (attributes for cache consistency).

Steps:
1. **Argument Definition**: The module defines `Args` to encapsulate the file handle and the byte range `[offset, offset + count)` (or `[offset, EOF)` if count is 0).
2. **Result Definition**: The module defines `Success` and `Fail`. Both include `WccData` to ensure the client can update its attribute cache regardless of the operation's outcome.
3. **Verifier Inclusion**: The `Success` struct includes a `verifier` of type `vfs::write::Verifier`. This allows the client to compare the verifier returned here with the one returned from a previous `WRITE` operation. If they differ, it implies a server reboot occurred, and the unstable data may have been lost.
4. **Trait Declaration**: The `Commit` trait declares the asynchronous `commit` method, enforcing that any VFS implementation must accept these arguments and return these results.

Edge Cases:
- **Count is Zero**: If `count` in `Args` is `0`, the operation implies flushing data from `offset` to the end of the file.
- **Server Reboot**: The `verifier` in `Success` is the primary mechanism for the client to detect if the server rebooted after an unstable write. The implementation must ensure this verifier changes on reboot.

Complexity:
- **Time**: N/A (This module defines interfaces and data structures; it contains no executable logic).
- **Space**: O(1) for the defined structs.

Determinism:
- **Deterministic**: The module defines static types and interfaces.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: Used in `Success` and `Fail` to carry Weak Cache Consistency information. This ensures the client can synchronize its cached view of the file's attributes with the server's state.
 - **`Error`**: Used in `Fail` to report the specific failure reason (e.g., `IO`, `StaleFile`), mapping the VFS operation result to standard NFS status codes.

- **From `nfs_mamont::vfs::write`**:
 - **`Verifier`**: Used in `Success`. This is a fixed-size byte array acting as a cookie. Its presence in the `Commit` response is critical for the client to verify the continuity of the server's state relative to a previous `Write` operation.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: Used in `Args` to uniquely identify the file system object to be committed.

---

## 4. Data Model

Entities:
- **`Success`**: Represents a successful commit operation.
 - Fields: `file_wcc` (vfs::WccData), `verifier` (vfs::write::Verifier).
- **`Fail`**: Represents a failed commit operation.
 - Fields: `error` (vfs::Error), `file_wcc` (vfs::WccData).
- **`Args`**: Arguments for the commit operation.
 - Fields: `file` (file::Handle), `offset` (u64), `count` (u32).

Relations:
- **Composition**: `Args` contains `file::Handle`.
- **Association**: `Success` and `Fail` contain `vfs::WccData`.
- **Association**: `Success` contains `vfs::write::Verifier`.

Global Invariants:
- **Range Semantics**: If `Args.count` is `0`, the range to commit is implicitly defined as `Args.offset` to the end of the file.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type imported from the parent `vfs` module, used within the `Fail` struct.

Error Propagation Strategy:
- **Result Wrapping**: The `commit` method returns `Result<Success, Fail>`. The `Fail` struct wraps the `vfs::Error` along with `WccData`, ensuring that error responses still provide cache consistency information.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` variant returned (e.g., `IO` might be transient, `StaleFile` requires the client to look up the file again).

Panics:
- **Allowed**: No. This module defines data structures and interfaces; it does not contain logic that panics.

---

## 6. Traits

List which external traits this module implements:
- **`Send`**: Implemented for `Commit` via the `#[trait_variant::make(Send)]` attribute macro.

List which traits this module defines:
- **`Commit`**: The trait defining the asynchronous commit operation interface for the VFS.

---

## 7. Overview

This module is used in order to **define the interface and data structures for the NFSv3 COMMIT procedure** within the `nfs_mamont` server. The system contains a complex architecture where the network layer (RPC) must communicate with the storage layer (VFS) using a strict contract defined by the NFS protocol. This module serves as the definition of that contract for commit operations.

The system requires a mechanism to ensure data integrity and consistency for clients that perform "unstable" writes (writes where the server acknowledges receipt but hasn't necessarily flushed data to disk). The `Commit` procedure allows the client to later request that this data be flushed to stable storage. Furthermore, because unstable writes reside in volatile memory (server cache), a server reboot could lose this data. To address this, the module integrates the `Verifier` from the `write` module. The client compares the verifier returned by `Commit` with the one from the previous `Write`; a mismatch indicates a server reboot, signaling the client that the data was lost and must be re-written.

A typical usage scenario of the system involves the RPC layer receiving a COMMIT request packet. It parses the file handle, offset, and count into an `Args` struct. It invokes the `commit` method on the VFS implementation. The VFS implementation ensures the specified byte range is persisted to the underlying storage. It returns a `Success` struct containing the `file_wcc` (to update the client's attribute cache) and a `verifier`. The RPC layer serializes this back to the client.

Inside the system, the following things happen and they use this module:
1.  **Protocol Compliance**: The `Commit` trait ensures that any storage backend plugged into the server supports the specific arguments and returns the specific result types required by the NFSv3 specification (RFC 1813).
2.  **State Verification**: By including the `verifier` in `Success`, the module enables the client-side logic to detect server reboots, which is essential for maintaining data consistency when using unstable writes.
3.  **Cache Consistency**: By including `WccData` in both `Success` and `Fail`, the module ensures that the client can update its attribute cache regardless of whether the commit succeeded or failed.

Without this module, the server would lack a standardized way to handle commit requests, leading to potential data loss scenarios where clients assume data is safe when it is not, or inability to verify server state continuity. This module centralizes these definitions, providing a clear contract for both the network layer and the storage implementors.