<!-- SPEC_HASH: f55056a7a477c29951115755f02c297ed3fea26aa76e645a25e8ac90b193dbd6 -->
# Module Specification

Module: mirrorfs::fs::read_impl
Rust File: mirror_fs/src/fs/read_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::SeekFrom`**: Used to specify the position for seeking within the file before reading.
- **`tokio::fs::File`**: Used to perform asynchronous file I/O operations (opening, reading, seeking) on the local filesystem that `MirrorFS` mirrors.
- **`tokio::io::{AsyncReadExt, AsyncSeekExt}`**: Used to extend `File` with asynchronous methods for reading data and seeking within the file.
- **`nfs_mamont::vfs::read`**: Used to import the `Read` trait, `Args`, `Success`, and `Fail` types. This module provides the implementation of the `Read` trait for `MirrorFS`, adhering to the NFSv3 contract.
- **`nfs_mamont::Buffer`**: Used as a generic bound `B` for the `Read` trait implementation. It represents the memory region allocated by the server that the implementation must fill with file data.
- **`super::MirrorFS`**: The parent struct for which this implementation is defined. The implementation relies on helper methods assumed to exist on `MirrorFS` (or its super traits) to resolve handles to paths, retrieve metadata, and convert errors.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement the `nfs_mamont::vfs::read::Read` trait for the `MirrorFS` backend.
- To translate an abstract NFS read request (identified by a file handle) into a concrete filesystem read operation on the local disk.
- To fill a server-allocated buffer (potentially non-contiguous) with file data efficiently using asynchronous I/O.

Inputs:
- **`args: read::Args`**: Contains the `file` handle, the byte `offset` to start reading from, and the `count` of bytes to read.
- **`data: B`**: A mutable buffer implementing the `Buffer` trait, provided by the server's allocator.

Outputs:
- **`Result<read::Success<B>, read::Fail>`**:
 - **`Success<B>`**: Contains the filled buffer `data` and a `head` with file attributes, bytes read, and EOF status.
 - **`Fail`**: Contains a `vfs::Error` and optional file attributes if the operation fails.

Steps:
1. **Handle Resolution**: The implementation calls `self.path_for_handle(&args.file).await` to translate the opaque NFS file handle into a concrete filesystem path. If this fails, it returns `Fail` with the error.
2. **Metadata Retrieval**: It calls `Self::metadata(&path)` to obtain the file's metadata. If this fails, it returns `Fail`.
3. **Attribute Conversion & Validation**: It converts metadata to `Attr` using `Self::attr_from_metadata` and validates that the file is a regular file using `Self::validate_regular`. If validation fails, it returns `Fail` with the attributes.
4. **File Opening**: It attempts to open the file asynchronously using `File::open(&path).await`. If opening fails, it maps the `std::io::Error` to a `vfs::Error` using `Self::io_error_to_vfs` and returns `Fail`.
5. **Range Calculation**: It calculates the effective read range:
 - `start`: `args.offset` clamped to `file_len`.
 - `end`: `args.offset + count` clamped to `file_len`.
 - `requested`: The number of bytes to read (`end - start`).
6. **Seeking**: It seeks the file cursor to `start` using `file.seek(SeekFrom::Start(start)).await`. If seeking fails, it returns `Fail`.
7. **Chunked Read Loop**: If `requested > 0`, it iterates over the mutable chunks of the provided buffer `data`:
 - For each chunk, it calculates the number of bytes to read (`to_read`), ensuring it does not exceed the remaining requested bytes.
 - It enters an inner loop calling `file.read(...)` to fill the chunk slice until `to_read` bytes are read or EOF (read returns 0) is reached.
 - It updates the total `read_count`.
 - If an I/O error occurs during reading, it returns `Fail`.
8. **Result Construction**: It returns `Success` containing the filled buffer, the file attributes, the `read_count`, and an `eof` flag. The `eof` flag is `true` if `start + read_count >= file_len`.

Edge Cases:
- **Offset Past EOF**: If `args.offset` is greater than or equal to the file length, `start` equals `file_len`, `requested` is 0, and the function returns `Success` with `count = 0` and `eof = true`.
- **Short Reads**: The loop handles short reads (where `file.read` returns fewer bytes than requested) by continuing to read until the chunk is full or EOF is reached.
- **Non-Contiguous Buffer**: The implementation correctly handles buffers that may consist of multiple disjoint memory segments by iterating via `chunks_mut`.

Complexity:
- **Time**: O(N) where N is the number of bytes read (`read_count`), dominated by disk I/O latency.
- **Space**: O(1) stack space (excluding the provided buffer `B`).

Determinism:
- **Deterministic**: The behavior is fully determined by the state of the filesystem and the input arguments.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::read`**:
 - **`Read` Trait Contract**: The module strictly adheres to the contract defined in the dependency. It takes ownership of `data: B` and returns it inside `Success<B>`. It ensures `file_attr` is populated in both success and failure cases to support Weak Cache Consistency.
 - **`Args` and `SuccessPartial`**: The module interprets `Args` (offset, count) to perform the I/O and constructs `SuccessPartial` (count, eof) based on the result of that I/O.

- **From `nfs_mamont::allocator`**:
 - **`Buffer` Trait (`chunks_mut`)**: The module relies on the `chunks_mut()` method to access the underlying memory of the buffer. This mechanism allows the implementation to write data into the buffer regardless of whether the memory is physically contiguous or split into multiple segments (e.g., a `Slice` composed of multiple `UnownedBuffer`s).

- **From `tokio::fs::File`**:
 - **Async I/O**: The module uses the asynchronous `read` and `seek` methods to perform non-blocking operations, which is essential for the high-performance nature of the `nfs_mamont` server.

---

## 4. Data Model

Entities:
- This module defines no public structs or enums. It operates entirely on types defined in its dependencies (`read::Args`, `read::Success`, `read::Fail`, `B: Buffer`).

Relations:
- **Implementation Relation**: `MirrorFS` implements `read::Read<B>`.

Global Invariants:
- **Buffer Integrity**: The `data` buffer returned in `Success` must contain exactly `read_count` bytes of valid file data starting from the calculated `start` offset.
- **EOF Semantics**: The `eof` flag in the result must accurately reflect whether the read operation consumed the entire remaining file content.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type returned within `read::Fail`. This includes errors like `IO`, `InvalidArgument`, `StaleFile`, etc.

Error Propagation Strategy:
- **Early Return**: The function uses a series of `match` statements to check intermediate results (path resolution, metadata, file open, seek, read). If any step fails, it immediately returns `Err(read::Fail { ... })`.
- **Error Mapping**: Standard `std::io::Error` instances from Tokio file operations are converted to `vfs::Error` using the helper `Self::io_error_to_vfs`.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` returned. For example, transient `IO` errors might be retryable by the client, while `StaleFile` or `InvalidArgument` typically require the client to re-lookup the file or fix the request.

Panics:
- **Allowed**: No explicit panics are present in this code. All error paths are handled via `Result`.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::read::Read<B>`** for `MirrorFS`.

List which traits this module defines:
- None.

---

## 7. Overview

This module is used in order to **bridge the abstract NFSv3 read interface with the concrete local filesystem capabilities of the `MirrorFS` backend**. The system contains a high-performance NFS server that abstracts storage behind the `Vfs` trait. This module is necessary because it provides the specific logic required to execute a read operation on a local disk, translating the generic "read from handle" command into specific "open path, seek, read" instructions.

A typical usage scenario of the system involves a client requesting to read a file. The RPC layer receives the request, allocates a buffer, and calls the `read` method on the `MirrorFS` instance. This module resolves the client's file handle to a local path (e.g., `/mnt/storage/file.txt`), opens the file, and fills the provided buffer with the requested data. It then returns the buffer along with file attributes and EOF status to the RPC layer, which sends the response back to the client.

Inside the system, the following things happen and they use this module:
1. **Handle Translation**: The module relies on `MirrorFS`'s internal state (accessed via `path_for_handle`) to map the opaque file handles used by the NFS protocol to actual file paths on the server's disk.
2. **Buffer Filling**: The module interacts with the server's memory allocator (via the `Buffer` trait) to fill the pre-allocated memory. The chunked iteration mechanism ensures that even if the allocator provides a buffer composed of multiple disjoint memory blocks, the data is written correctly without requiring a contiguous copy.
3. **Protocol Compliance**: The module ensures that the read operation strictly follows NFSv3 semantics, such as returning `0` bytes and `eof=true` if the offset is past the end of the file, and correctly calculating the EOF flag based on the bytes read versus the file size.

The critical aspect of this module is the **integration of Tokio's asynchronous file I/O with the `nfs_mamont` buffer model**. It ensures that reading from the disk does not block the server's main thread and that data is transferred directly into the memory managed by the server's allocator, minimizing copies and maximizing throughput.

**Assumptions**:
- The parent module `super::MirrorFS` provides the following helper methods, which are not defined in the provided snippet but are used in the code:
 - `async fn path_for_handle(&self, handle: &file::Handle) -> Result<PathBuf, vfs::Error>`
 - `fn metadata(path: &Path) -> Result<std::fs::Metadata, vfs::Error>`
 - `fn attr_from_metadata(meta: &std::fs::Metadata) -> file::Attr`
 - `fn validate_regular(attr: &file::Attr) -> Result<(), vfs::Error>`
 - `fn io_error_to_vfs(error: &std::io::Error) -> vfs::Error`