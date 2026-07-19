<!-- SPEC_HASH: 207c28d75ddf092e2c2b635464ea8c747841c0940908095a00fd76efc192c87a -->
# Module Specification

Module: mirrorfs::fs::write_impl
Rust File: mirror_fs/src/fs/write_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**: Used for `IoSlice` and `SeekFrom`. `IoSlice` is utilized to perform vectored I/O, allowing the module to write data from multiple non-contiguous memory buffers in a single system call. `SeekFrom` defines the position within the file where writing should begin.
- **`tokio::fs::OpenOptions`**: Used to asynchronously open the target file on the local filesystem. It is configured to allow writing (`write(true)`) while preserving existing content (`truncate(false)`).
- **`tokio::io` (AsyncSeekExt, AsyncWriteExt)**: Provides extension methods for asynchronous file operations. `AsyncSeekExt::seek` is used to move the file cursor to the specified offset, and `AsyncWriteExt::write_vectored` is used to write the data efficiently. `sync_data` and `sync_all` are used to enforce data stability requirements.
- **`nfs_mamont::vfs`**: Used to import the `write` module (which defines the `Write` trait and associated types) and the `vfs::Error` type. This module implements the `vfs::write::Write` trait for `MirrorFS`, thereby fulfilling the contract required by the NFS server core.
- **`nfs_mamont::Buffer`**: Used as a generic bound `B` for the `write` implementation. This allows the module to accept data in any buffer format that satisfies the server's memory management interface (e.g., a `Slice` from the memory pool).
- **`super::MirrorFS`**: The type for which the `Write` trait is being implemented. The implementation relies on several internal methods of `MirrorFS` (assumed to exist) to map handles to paths, convert metadata, and generate verifiers.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Asynchronous Write Execution with WCC Tracking

**Intent:**
To perform a write operation on the local filesystem that strictly adheres to the NFSv3 protocol semantics. This involves translating an abstract file handle to a concrete path, managing file offsets, enforcing data stability guarantees (syncing), and collecting Weak Cache Consistency (WCC) data before and after the operation to allow the client to validate its cache.

**Inputs:**
- `&self`: A reference to the `MirrorFS` instance.
- `args: write::Args<B>`: Arguments containing the file handle, offset, data size, stability requirement (`StableHow`), and the data buffer itself.

**Outputs:**
- `Result<write::Success, write::Fail>`:
 - `Success`: Contains the count of bytes written, the achieved stability level, a write verifier, and the `file_wcc` (post-operation attributes).
 - `Fail`: Contains the error that occurred and the `wcc_data` (attributes captured before the failure).

**Steps:**
1. **Path Resolution**: The module calls `self.path_for_handle(&args.file)` to convert the abstract NFS file handle into a local filesystem path. If this fails, it returns `Fail` immediately with empty WCC data.
2. **Pre-Operation State Capture**: It retrieves the file's metadata using `std::fs::symlink_metadata` (blocking). This metadata is converted into `WccAttr` and stored as the `before` state for WCC reporting.
3. **Validation**: It checks if the target is a regular file using `Self::validate_regular`. If not, it returns `Fail` including the captured `before` state.
4. **File Opening**: It opens the file asynchronously using `OpenOptions::new().write(true).truncate(false).open(&path)`. If opening fails, it returns `Fail` with the `before` state.
5. **Data Preparation**: It prepares a vector of `IoSlice` from the input buffer `args.data`. It slices the data to match exactly `args.size`, ignoring any excess data in the buffer.
6. **Seeking**: It moves the file cursor to `args.offset` using `file.seek(SeekFrom::Start(args.offset))`. Failure here results in a `Fail`.
7. **Writing**: It writes the prepared data slices using `file.write_vectored(&data)`. The number of bytes written (`n`) is captured. Failure results in a `Fail`.
8. **Stability Enforcement**: Based on `args.stable`:
 - `Unstable`: No action is taken.
 - `DataSync`: `file.sync_data().await` is called (flushes data).
 - `FileSync`: `file.sync_all().await` is called (flushes data and metadata).
 If the sync operation fails, a `Fail` is returned.
9. **Response Construction**: It calculates the `after` state for WCC by querying the file metadata again. It constructs a `Success` struct containing the byte count, the requested stability, a verifier (via `self.write_verifier()`), and the WCC data.

**Edge Cases:**
- **Handle Resolution Failure**: If the file handle is invalid or stale, `path_for_handle` fails, and the operation aborts with `BadFileHandle` (or similar) and empty WCC.
- **Short Writes**: The code relies on `write_vectored` return value. If the OS writes fewer bytes than requested (though rare with local disks), `count` reflects the actual number written.
- **Sync Failures**: If data is written but the subsequent `sync` fails, the operation is considered a failure (`Fail`), but the data may reside in the OS cache. The WCC data in the `Fail` response reflects the state *before* the write attempt.

**Complexity:**
- **Time**: Dominated by disk I/O (seek, write, sync). O(1) for metadata operations, O(N) for writing N bytes.
- **Space**: O(1) auxiliary space (excluding the input buffer and the `Vec<IoSlice>` which holds references, not data).

**Determinism:**
- **Non-deterministic**: Depends on the state of the local filesystem, disk latency, and potential concurrent modifications to the file metadata.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::write`**:
 - **`Write` Trait**: The module implements this trait, specifically the `write` method. This dictates the input arguments (`Args<B>`) and the return types (`Success`, `Fail`).
 - **`StableHow` Enum**: The module matches on this enum to determine whether to call `sync_data`, `sync_all`, or skip syncing entirely.
 - **`Verifier`**: The module generates this (via `self.write_verifier()`) to include in the `Success` response, allowing the client to detect server reboots.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: The module constructs this structure at the end of the operation (or on failure) to return the `before` and `after` attributes of the file. This is critical for the client's cache coherency.
 - **`Error`**: The module converts `std::io::Error` (from Tokio/Std) into `vfs::Error` (via `Self::io_error_to_vfs`) to report failures in a protocol-compliant manner.

- **From `nfs_mamont::allocator`**:
 - **`Buffer` Trait**: The module accepts the data to be written as a generic type `B` implementing `Buffer`. It uses `chunks()` to iterate over the data segments, creating `IoSlice` pointers to them without copying the data.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on entities defined in dependencies (`Args`, `Success`, `Fail`, `MirrorFS`).

Relations:
- **Implementation**: `MirrorFS` implements `vfs::write::Write<B>`.

Global Invariants:
- **WCC Consistency**: If the operation fails after the file is opened, the `wcc_data` returned in `Fail` must contain the `before` attributes captured at the start of the operation. The `after` attributes should be `None` in this case (as handled by `Self::wcc_data` logic).
- **Data Integrity**: The number of bytes reported in `Success.count` must match the return value of the underlying `write_vectored` call.

---

## 5. Error Model

Error Types:
- **`vfs::Error`**: The canonical error type returned within `write::Fail`. This includes errors like `IO`, `NoSpace`, `Access`, etc.

Error Propagation Strategy:
- **Conversion**: `std::io::Error` from Tokio file operations are converted to `vfs::Error` using an internal helper `Self::io_error_to_vfs`.
- **Early Return**: The function uses the `?` operator (or explicit `return Err`) to propagate errors immediately upon failure, ensuring that WCC data is preserved in the error response.

Recoverability:
- **Dependent on Error**: Transient errors like `IO` might be recoverable by the client retrying. Errors like `Access` or `IsDir` indicate persistent state issues requiring client intervention.

Panics:
- **Allowed**: No explicit panics are triggered in this code. However, internal helpers like `path_for_handle` or `validate_regular` (assumed to exist on `MirrorFS`) could potentially panic if invariants are violated, though this is not observable in this specific file.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::write::Write<B>`** for `MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete implementation of the NFSv3 WRITE procedure for the `MirrorFS` backend**. The system requires a way to translate high-level NFS write requests—characterized by opaque file handles, specific offsets, and stability guarantees—into low-level asynchronous filesystem operations on the local disk.

This system contains a **local filesystem mirror** (`MirrorFS`) that acts as a storage backend for the NFS-Mamont server. The `MirrorFS` struct is responsible for mapping NFS file handles to actual paths on the host OS. This module is necessary because it defines exactly *how* data is persisted: it handles the nuances of opening files without truncation, seeking to specific offsets, and ensuring that data is flushed to disk according to the client's stability requirements (`Unstable`, `DataSync`, `FileSync`).

A typical usage scenario of the system involves a client sending a WRITE request for a specific file range. The RPC layer parses this into `write::Args`. The `MirrorFS` implementation receives these arguments, resolves the handle to a path (e.g., `/mnt/export/file.txt`), and opens the file. It then seeks to the requested offset and writes the data. If the client requested `FileSync`, the module ensures that both the file content and its metadata (like modification time) are committed to stable storage before returning. Finally, it captures the file's new attributes to construct the `WccData`, allowing the client to update its cache.

Inside the system, the following things happen and they use this module:
1.  **Protocol Compliance**: The module ensures that the backend respects the `StableHow` contract. Without this, the server might claim data is written when it is only in volatile memory, risking data loss on power failure.
2.  **Cache Coherency**: By capturing metadata *before* and *after* the write, the module provides the `WccData` required by the NFS protocol. This allows clients to validate their cached attribute information without performing expensive separate `GETATTR` calls.
3.  **Efficient Data Transfer**: The module utilizes the `Buffer` trait and `IoSlice` to perform vectored writes. This allows the server to write data that might be scattered across multiple memory buffers (e.g., from a zero-copy network stack) directly to the file in a single system call, minimizing CPU overhead and memory copies.

**Assumptions**: The code relies on several helper methods on `MirrorFS` (e.g., `path_for_handle`, `wcc_attr_from_metadata`, `io_error_to_vfs`, `write_verifier`) which are not defined in the provided public interfaces of `mirrorfs::fs` but are assumed to exist as internal implementation details to support this logic.