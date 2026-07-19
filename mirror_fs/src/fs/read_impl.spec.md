<!-- SPEC_HASH: f55056a7a477c29951115755f02c297ed3fea26aa76e645a25e8ac90b193dbd6 -->
# Module Specification

Module: mirrorfs::fs::read_impl
Rust File: mirror_fs/src/fs/read_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs::File`**: Used to perform asynchronous file I/O operations on the local file system. It provides the `open`, `seek`, and `read` capabilities required to fulfill the NFS read request.
- **`tokio::io::{AsyncReadExt, AsyncSeekExt}`**: Used to extend `tokio::fs::File` with asynchronous methods `read` and `seek`, which are essential for non-blocking file access.
- **`nfs_mamont::vfs::read`**: Used to import the `Read` trait, `Args`, `Success`, and `Fail` types. This module provides the implementation of the `Read` trait for `MirrorFS`, defining the contract that this code satisfies.
- **`nfs_mamont::Buffer`**: Used as a generic bound `B` for the `read` function. It provides the `chunks_mut` method, which allows the implementation to write file data directly into the memory regions managed by the server's allocator.
- **`super::MirrorFS`**: The struct for which the `Read` trait is implemented. The implementation relies on several internal methods of `MirrorFS` (or its parent module) such as `path_for_handle`, `metadata`, `attr_from_metadata`, `validate_regular`, and `io_error_to_vfs` to translate NFS concepts into local file system operations.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement the `nfs_mamont::vfs::read::Read` trait for the `MirrorFS` backend.
- To translate an abstract NFS read request (handle, offset, count) into a concrete asynchronous file read operation on the local disk.
- To populate the server-provided buffer (`data: B`) efficiently by iterating over its mutable chunks, ensuring data is written directly into the memory managed by the allocator.

Inputs:
- **`args: read::Args`**: Contains the `file` handle, the byte `offset` to start reading from, and the `count` of bytes to read.
- **`data: B`**: A buffer implementing the `Buffer` trait. This buffer is pre-allocated by the server and passed to the implementation to be filled.

Outputs:
- **`Result<read::Success<B>, read::Fail>`**:
 - **`Success<B>`**: Contains the filled buffer, the number of bytes read, the EOF status, and the file attributes.
 - **`Fail`**: Contains the error encountered and optional file attributes.

Steps:
1. **Handle Resolution**: Calls `self.path_for_handle(&args.file).await` to convert the abstract NFS file handle into a concrete local filesystem path. If this fails, returns `Fail` immediately.
2. **Metadata Retrieval**: Calls `Self::metadata(&path)` to obtain file metadata (size, type, etc.). If this fails, returns `Fail`.
3. **Attribute Conversion**: Converts the raw metadata into `file::Attr` using `Self::attr_from_metadata`.
4. **Type Validation**: Calls `Self::validate_regular(&attr)` to ensure the target is a regular file. If not, returns `Fail` with `InvalidArgument` (implied).
5. **File Opening**: Attempts to open the file using `File::open(&path).await`. If an OS error occurs, it is mapped to a VFS error via `Self::io_error_to_vfs`, and `Fail` is returned.
6. **Range Calculation**: Calculates the effective read window:
 - `start`: `args.offset` clamped to `file_len`.
 - `end`: `args.offset + count` clamped to `file_len`.
 - `requested`: `end - start`.
7. **Seeking**: Seeks the file cursor to `start` using `file.seek(SeekFrom::Start(start)).await`. Returns `Fail` on error.
8. **Buffer Filling Loop**:
 - Iterates over `data.chunks_mut()` to get mutable slices of the buffer.
 - For each chunk, reads data from the file using `file.read(...).await` until the chunk is full or EOF is reached.
 - Updates `read_count` and `remaining` bytes to track progress.
 - If `file.read` returns `0` (EOF), the loop terminates early.
 - If an IO error occurs during reading, returns `Fail`.
9. **Result Construction**: Returns `Success` containing the filled buffer, the total `read_count`, an `eof` flag (true if the read reached the end of the file), and the file attributes.

Edge Cases:
- **Offset Past End**: If `args.offset` is greater than or equal to `file_len`, `start` equals `end`, `requested` is 0, and the function returns `Success` with `count: 0` and `eof: true` without performing I/O.
- **Short Reads**: If the file has fewer bytes remaining than requested, the loop terminates when `file.read` returns 0, resulting in a `count` less than `args.count`.
- **Non-regular Files**: Explicitly checked via `validate_regular`. Directories or special files result in a `Fail`.

Complexity:
- **Time**: O(N), where N is the number of bytes actually read from the disk.
- **Space**: O(1) auxiliary stack space. The buffer `B` is passed by ownership.

Determinism:
- **Deterministic**: The behavior is fully determined by the state of the file system at the time of the call and the input arguments.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::read`**:
 - **`Read` Trait**: Defines the `async fn read` signature that this module implements. It enforces the pattern of taking ownership of a buffer `B` and returning it inside `Success<B>`.
 - **`Args` and `Success`**: Define the structure of the input (handle, offset, count) and output (data, attributes, EOF flag).
 - **`Fail`**: Defines the error wrapper that requires a `vfs::Error` and optional attributes.

- **From `nfs_mamont::allocator`**:
 - **`Buffer` Trait**: The critical mechanism here is `chunks_mut()`. The implementation uses this to iterate over the buffer's internal memory segments. This allows the `MirrorFS` implementation to write data directly into the underlying memory (potentially non-contiguous) without requiring the buffer to expose a single contiguous `&mut [u8]`.

- **From `super::MirrorFS` (Assumed based on usage)**:
 - **`path_for_handle`**: Provides the mapping from the NFS-specific `Handle` to a local `PathBuf`. This is the bridge between the protocol layer and the storage layer.
 - **`io_error_to_vfs`**: Provides the translation layer from standard `std::io::Error` to the NFS-specific `vfs::Error` enum, ensuring that OS-level failures (e.g., `NotFound`, `PermissionDenied`) are correctly mapped to protocol status codes.

---

## 4. Data Model

Entities:
- This module defines no new public structs or enums. It implements the `read::Read` trait for `MirrorFS`.

Relations:
- **Implementation**: `MirrorFS` implements `read::Read<B>`.

Global Invariants:
- **Buffer Capacity**: The implementation will never write more bytes into `data` than `data.len()`.
- **EOF Consistency**: The `eof` flag in the return value is `true` if and only if the last byte read was the last byte of the file, or if the read offset was at or past the end of the file.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped inside `read::Fail`. Specific errors depend on the outcome of `path_for_handle`, `metadata`, `validate_regular`, `File::open`, `seek`, and `read`.

Error Propagation Strategy:
- **Early Return**: The function uses the `?` operator or explicit `match` blocks to return `read::Fail` immediately upon encountering any error during path resolution, metadata fetching, validation, or I/O operations.
- **Context Preservation**: When returning `Fail`, the implementation attempts to include `file_attr` (post-operation attributes) if they were successfully retrieved before the failure occurred.

Recoverability:
- **Dependent on Error**: Recoverability is determined by the specific `vfs::Error` variant returned. For example, `IO` errors might be transient, while `StaleFile` or `InvalidArgument` typically require client intervention (e.g., re-lookup).

Panics:
- **Allowed**: No. The code handles all potential errors (IO, lookup) via `Result` types.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::read::Read<B>`** for `MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete storage backend logic for reading files** within the `MirrorFS` implementation of the NFS-Mamont server. The system contains a high-performance NFS server that abstracts storage behind the `Vfs` trait. This module is necessary because it connects the generic, protocol-agnostic `Read` trait definition to the specific reality of reading files from the local disk using Tokio's asynchronous runtime.

A typical usage scenario of the system involves an NFS client sending a `READ` request for a specific file handle. The server's RPC layer dispatches this to `MirrorFS::read`. This module resolves the handle to a local path (e.g., `/var/nfs/root/file.txt`), opens the file, and seeks to the requested offset. It then fills the pre-allocated buffer (provided by the server's memory pool) with file data. By using `chunks_mut()`, it efficiently handles the buffer's internal memory layout. Finally, it returns the data along with file attributes and an EOF flag to the RPC layer, which formats the response for the client.

Inside the system, the following things happen and they use this module:
1. **Protocol Translation**: The module translates the NFS "read at offset X for Y bytes" semantic into the corresponding `std::fs::File` operations (`seek` and `read`).
2. **Buffer Integration**: The module acts as the consumer of the `Buffer` trait. It demonstrates how a backend should interact with the server's memory allocator by iterating over `chunks_mut`, ensuring zero-copy (or single-copy) data transfer from the disk to the network buffer.
3. **Error Normalization**: The module uses `io_error_to_vfs` to ensure that standard OS errors (like "File not found") are converted into the specific NFS error codes required by the protocol, maintaining the abstraction layer.

The critical aspect of this module is the **handling of the read loop**. It must correctly handle partial reads (where `file.read` returns fewer bytes than requested) and EOF conditions, ensuring that the `count` and `eof` fields in the `Success` response accurately reflect the state of the file, as this is crucial for NFS client caching logic.