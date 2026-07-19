<!-- SPEC_HASH: 207c28d75ddf092e2c2b635464ea8c747841c0940908095a00fd76efc192c87a -->
# Module Specification

Module: mirrorfs::fs::write_impl
Rust File: mirror_fs/src/fs/write_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**: Used for `IoSlice` to facilitate vectored I/O operations (writing multiple non-contiguous buffers in a single system call) and `SeekFrom` to specify the offset within the file.
- **`tokio::fs` and `tokio::io`**: Used for asynchronous file system operations. `OpenOptions` opens the file, `AsyncSeekExt` provides the `seek` method, and `AsyncWriteExt` provides `write_vectored` and synchronization methods (`sync_data`, `sync_all`). This allows the implementation to perform non-blocking I/O, which is essential for the asynchronous runtime of the NFS server.
- **`nfs_mamont::vfs`**: Used to import the `write` module (containing the `Write` trait, `Args`, `Success`, `Fail`, and `StableHow`) and the parent `vfs` module (containing `WccData` and `Error`). These define the contract that this implementation must satisfy.
- **`nfs_mamont::Buffer`**: Used as a generic bound `B` for the `Write` trait implementation. This allows the module to accept data in any buffer format that satisfies the server's memory management interface (e.g., a `Slice` from the custom allocator).
- **`super::MirrorFS`**: The struct for which this trait implementation is defined. The implementation relies on several methods of `MirrorFS` (e.g., `path_for_handle`, `wcc_attr_from_metadata`, `io_error_to_vfs`) which are assumed to exist based on their usage in the code, though they were not explicitly listed in the provided partial specification of `MirrorFS`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `vfs::write::Write` for `MirrorFS`

**Intent:**
To provide a concrete, asynchronous implementation of the NFSv3 WRITE procedure for the `MirrorFS` backend. This involves translating the abstract VFS write request (which includes stability requirements and vectored data) into specific Tokio file system operations on a local path, while managing Weak Cache Consistency (WCC) data and error mapping.

**Inputs:**
- `&self`: A reference to the `MirrorFS` instance.
- `args: write::Args<B>`: Arguments containing the file handle, offset, requested size, stability requirement (`StableHow`), and the data buffer.

**Outputs:**
- `Result<write::Success, write::Fail>`:
    - `Success`: Contains the count of bytes written, the achieved stability level, a verifier, and WCC data.
    - `Fail`: Contains a `vfs::Error` and WCC data.

**Steps:**
1.  **Path Resolution**: The implementation calls `self.path_for_handle(&args.file)` to convert the abstract NFS file handle into a concrete local filesystem path. If this fails, it returns a `Fail` with empty WCC data.
2.  **Pre-Operation Metadata Capture**: It retrieves the file's metadata using `std::fs::symlink_metadata` (blocking call) to establish the "before" state for WCC. It converts this metadata into `WccAttr` using a helper method.
3.  **Validation**: It checks if the target is a regular file using `Self::validate_regular`. If not (e.g., it's a directory), it returns a `Fail` with the captured "before" WCC data.
4.  **File Opening**: It opens the file using `tokio::fs::OpenOptions` with write enabled and truncation disabled. If opening fails, it maps the I/O error to a `vfs::Error` and returns a `Fail`.
5.  **Data Preparation**: It prepares a `Vec<IoSlice>` from the input buffer `args.data`. It slices the data according to `args.size`, ensuring that only the requested amount of data is passed to the write operation, even if the buffer is larger.
6.  **Seeking**: It seeks the file cursor to `args.offset` using `AsyncSeekExt`. If seeking fails, it returns a `Fail`.
7.  **Vectored Write**: It performs the write operation using `write_vectored` with the prepared slices. This allows efficient writing of scattered memory segments. It captures the number of bytes actually written.
8.  **Stability Enforcement**: Depending on `args.stable`:
    - `Unstable`: No action is taken.
    - `DataSync`: `file.sync_data().await` is called to flush data.
    - `FileSync`: `file.sync_all().await` is called to flush data and metadata.
    If synchronization fails, it returns a `Fail`.
9.  **Response Construction**: It calculates the final WCC data (including "after" attributes) using `Self::wcc_data`. It returns `Success` containing the byte count, the requested stability, a verifier from `self.write_verifier()`, and the WCC data.

**Edge Cases:**
- **Handle Resolution Failure**: If the file handle is invalid or stale, `path_for_handle` fails, and the operation aborts with `NFS3ERR_STALE` (or similar).
- **Short Writes**: The code returns the actual number of bytes written (`n`) as reported by `write_vectored`. It does not loop to ensure all bytes are written if the first call returns a short count, which is standard behavior for a single NFS WRITE procedure invocation (the client is responsible for re-issuing the request for the remaining data).
- **Synchronous Metadata Capture**: The use of `std::fs::symlink_metadata` (blocking) inside an async function might block the executor thread. This is a specific implementation detail of `MirrorFS`.

**Complexity:**
- **Time**: Dominated by disk I/O (open, seek, write, sync). O(1) for in-memory processing of arguments.
- **Space**: O(N) for the `Vec<IoSlice>` where N is the number of chunks in the input buffer.

**Determinism:**
- **Non-deterministic**: Depends on the state of the local filesystem (permissions, disk space, existing file content) and the success of I/O operations.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::write`**:
    - **`Write` Trait**: Defines the asynchronous interface (`write`) that this module implements. It dictates the input arguments (`Args`) and the structure of the result (`Success`/`Fail`).
    - **`StableHow` Enum**: Determines the synchronization logic (whether to call `sync_data`, `sync_all`, or skip syncing) within the implementation.
    - **`Verifier`**: The implementation relies on `self.write_verifier()` (assumed method on `MirrorFS`) to generate this value for the `Success` response.

- **From `nfs_mamont::vfs`**:
    - **`WccData`**: The implementation is responsible for populating this structure with "before" and "after" attributes. It captures "before" attributes immediately after resolving the path and calculates "after" attributes at the end of the operation.
    - **`Error`**: The implementation converts `std::io::Error` from Tokio file operations into `vfs::Error` variants using `Self::io_error_to_vfs`.

- **From `tokio::io`**:
    - **`AsyncWriteExt` and `AsyncSeekExt`**: Provide the methods (`write_vectored`, `seek`) used to perform the actual data manipulation on the file handle.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It implements the `write` function for the existing `MirrorFS` struct.

Relations:
- **Implementation**: `MirrorFS` implements `vfs::write::Write<B>`.

Global Invariants:
- **WCC Consistency**: If the file is successfully opened and modified, the `file_wcc` in the `Success` response must contain valid "before" attributes (captured prior to modification) and "after" attributes (captured post-modification).
- **Stability Guarantee**: If `args.stable` is `DataSync` or `FileSync`, the function guarantees that the data (and metadata) is committed to stable storage before returning `Success`, unless an error occurs.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type returned within `write::Fail`. This includes errors like `IO`, `NoSpace`, `Access`, `StaleFile`, etc.

Error Propagation Strategy:
- **Conversion**: `std::io::Error` from Tokio operations is converted to `vfs::Error` via `Self::io_error_to_vfs`.
- **Early Return**: The function uses the `?` operator and explicit `match`/`if let` blocks to return `write::Fail` immediately upon encountering an error at any stage (resolution, validation, open, seek, write, sync).
- **WCC Preservation**: Even when returning an error, the function attempts to provide `WccData` (specifically the "before" state if available) to allow the client to update its cache.

Recoverability:
- **Dependent on Error**: Transient errors like `IO` might be recoverable by the client retrying. Permanent errors like `Access` or `StaleFile` require client intervention (fix permissions or re-lookup the file).

Panics:
- **Allowed**: No explicit panics in this code. However, if the assumed helper methods on `MirrorFS` (e.g., `path_for_handle`) panic, this function will propagate the panic.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::write::Write<B>`** for `MirrorFS`.

---

## 7. Overview

This module is used in order to **bridge the abstract NFS VFS write interface to the concrete Tokio asynchronous file system implementation** for the `MirrorFS` backend. The system requires a way to execute write operations that conform to the NFSv3 protocol (handling vectored data, specific offsets, and stability guarantees) on top of a standard local file system.

This module is necessary because the generic `nfs_mamont::vfs::write::Write` trait only defines the *contract* (what arguments are taken and what results are returned). It does not define *how* the data is actually written to disk. The `MirrorFS` type acts as a local filesystem mirror, and this module provides the specific logic to fulfill that contract using Tokio's async I/O primitives.

A typical usage scenario involves an NFS client sending a WRITE request. The RPC layer parses this into `write::Args<B>` and calls the `write` method on the `MirrorFS` instance. This module then translates that request into a sequence of local operations: finding the file path, checking if it's a regular file, opening it, seeking to the correct position, writing the data, and ensuring the data is flushed to disk if the client requested it. It also gathers file attributes before and after the operation to return Weak Cache Consistency (WCC) data, which helps the client validate its cache.

Inside the system, the following things happen and they use this module:
1.  **Protocol Enforcement**: The module enforces the `StableHow` requirement by explicitly calling `sync_data` or `sync_all` based on the client's request, ensuring data durability guarantees are met.
2.  **Efficient Data Transfer**: By using `write_vectored` and `IoSlice`, the module efficiently handles data that might be stored in non-contiguous memory chunks (common in zero-copy network architectures), avoiding unnecessary memory copying.
3.  **Error Mapping**: It translates low-level OS errors (e.g., "No space left on device") into standard NFS error codes, ensuring the client receives meaningful error responses.

The critical aspect of this module is the **integration of async I/O with NFS semantics**. It must carefully manage the side effects (metadata changes, disk flushing) to match the strict state consistency requirements of the NFS protocol, while leveraging the non-blocking capabilities of the Tokio runtime.

**Uncertainty**: The provided specification for `super::MirrorFS` did not list the methods `path_for_handle`, `wcc_attr_from_metadata`, `attr_from_metadata`, `validate_regular`, `io_error_to_vfs`, `wcc_data`, or `write_verifier`. The analysis assumes these methods exist on `MirrorFS` based on their usage in the source code (`self.method_name(...)`).