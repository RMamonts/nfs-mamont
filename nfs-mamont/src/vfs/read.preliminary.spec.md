<!-- SPEC_HASH: 7a442ffdfb84cefce07313b3c3ee85ea577b30d81d04a22bbc3ff60803cc05e6 -->
# Module Specification

Module: nfs_mamont::vfs::read
Rust File: src/vfs/read.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::allocator::Buffer`**: Used as the generic type parameter `B` in the `Read` trait and `Success` struct. It represents the memory region into which file data must be written. The use of this trait allows the VFS implementation to interact with memory managed by the server's custom allocator without knowing the specific allocation strategy.
- **`crate::vfs`**: Used to import the `Error` enum, which is wrapped in the `Fail` struct to report I/O or protocol errors to the caller.
- **`super::file`**: Used to import `Handle` (to identify the target file in `Args`), `Attr` (to return post-operation file metadata in `Success` and `Fail`), and `Type` (referenced in documentation to specify that the handle must refer to a regular file).
- **`super::fs_info`**: Referenced in documentation for the `Args::count` field to define the constraint that the read size must not exceed the server's `read_max` value.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the asynchronous interface for the NFSv3 `READ` procedure.
- To enforce a specific memory ownership model where the server allocates the buffer (via `Buffer`) and passes it to the VFS implementation to be filled, ensuring zero-copy or controlled-copy semantics.
- To standardize the return values, including file attributes and EOF status, as required by the NFSv3 protocol.

Inputs:
- **`args: Args`**: Contains the `file::Handle` identifying the file, a `u64` offset indicating the starting position, and a `u32` count indicating the number of bytes to read.
- **`data: B`**: An instance of a type implementing the `Buffer` trait, representing the pre-allocated memory region where the read data must be stored.

Outputs:
- **`Result<Success<B>, Fail>`**:
  - **`Success<B>`**: Contains the filled `data` buffer and a `SuccessPartial` struct with metadata (attributes, byte count, EOF flag).
  - **`Fail`**: Contains a `vfs::Error` and optional file attributes.

Steps:
1. **Argument Validation**: The implementation must verify that the `file` handle in `Args` refers to a regular file (`file::Type::Regular`). If not, it returns `Fail` with `vfs::Error::InvalidArgument`.
2. **Offset Check**: If `Args.offset` is greater than or equal to the file size, the operation returns `Success` with `count` set to 0 and `eof` set to true. The buffer `data` is likely returned empty or unmodified.
3. **Zero-Length Read**: If `Args.count` is 0, the operation returns `Success` immediately with `count` set to 0.
4. **Data Transfer**: The implementation reads up to `Args.count` bytes starting at `Args.offset` from the file. The data is written into the provided `data` buffer.
5. **Result Construction**:
   - `SuccessPartial` is populated with the current file attributes (`file_attr`), the actual number of bytes read (`count`), and a boolean indicating if the end-of-file was reached (`eof`).
   - `Success` is constructed wrapping the `SuccessPartial` and the filled `data` buffer.
6. **Error Handling**: If an error occurs (e.g., permission denied, I/O error), `Fail` is returned containing the error and, if available, the post-operation file attributes.

Edge Cases:
- **Short Read**: If `Args.count` exceeds the server's `read_max` (from `fs_info`), the server may return fewer bytes than requested.
- **EOF Handling**: Reading exactly to the end of the file or beyond it results in `eof: true`.
- **Invalid Handle**: Passing a handle for a directory or other non-regular file type results in `InvalidArgument`.

Complexity:
- **Time**: O(N) where N is the number of bytes read, determined by the underlying storage implementation.
- **Space**: O(1) additional space, as the buffer is pre-allocated and passed by ownership.

Determinism:
- Deterministic. The output is strictly determined by the file system state at the time of the call and the provided arguments.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::allocator`**:
  - **`Buffer` Trait**: The `Read` trait relies on the `Buffer` trait to abstract the memory destination. The `chunks_mut` method of `Buffer` is implicitly required by the implementation to write data into the buffer's memory regions. The `read` interface transfers ownership of the `Buffer` to the implementation and back to the caller, ensuring the memory lifecycle is managed via RAII.
- **From `nfs_mamont::vfs::file`**:
  - **`Handle`**: Acts as the opaque key to retrieve the file object. The `read` operation does not interpret the handle internals but passes it to the underlying VFS driver.
  - **`Attr`**: The module requires the VFS implementation to retrieve the current file attributes post-read to populate `SuccessPartial.file_attr` and `Fail.file_attr`. This is essential for NFSv3 cache consistency.

---

## 4. Data Model

Entities:
- **`Args`**: A structure encapsulating the parameters for a read operation: `file` (Handle), `offset` (u64), and `count` (u32).
- **`Success<B>`**: A structure wrapping the result of a successful read. It contains `head` (SuccessPartial) and `data` (B).
- **`SuccessPartial`**: A structure containing metadata about the read operation: `file_attr` (Option<file::Attr>), `count` (u32), and `eof` (bool).
- **`Fail`**: A structure representing a failed read operation. It contains `error` (vfs::Error) and `file_attr` (Option<file::Attr>).

Relations:
- **`Args` → `file::Handle`**: Arguments reference a file handle.
- **`Success` → `SuccessPartial`**: Success contains partial metadata.
- **`Success` → `B`**: Success owns the filled buffer.
- **`Fail` → `vfs::Error`**: Fail wraps a VFS error.

Global Invariants:
- **Count Consistency**: The `count` field in `SuccessPartial` must be less than or equal to the `count` field in `Args`.
- **EOF Logic**: `eof` must be true if and only if the read operation started at or ended at the file's end of file marker.
- **Buffer Capacity**: The provided buffer `B` must have a capacity (`len()`) sufficient to hold the requested `count` bytes, or the implementation must handle short reads gracefully.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Enumerates standard NFSv3 and VFS errors (e.g., `InvalidArgument`, `IO`, `Access`).

Error Propagation Strategy:
- The `read` method returns a `Result<Success<B>, Fail>`. The `Fail` struct wraps the `vfs::Error` along with optional file attributes.

Recoverability:
- Recoverable. The caller receives the error in the `Fail` struct and can take appropriate action (e.g., logging, retrying, or returning the error to the NFS client).

Panics:
- Allowed: No (at the interface level). Implementations should not panic; they should return `Fail`.

---

## 6. Traits

List which external traits this module implements:
- **`Read<B: Buffer>`**: The primary trait defining the read operation. It is transformed into a Send-safe object-safe trait via `#[trait_variant::make(Send)]`.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the contract for reading file data within the NFSv3 server implementation, specifically ensuring that data is transferred directly into buffers managed by the server's high-performance allocator. The system contains a complex Virtual File System (VFS) abstraction layer that sits between the NFS protocol handlers and the actual storage backend. A typical usage scenario of the system involves the NFS server receiving a `READ` request from a client. The server allocates a buffer using the `nfs_mamont::allocator` (which pre-allocates fixed memory chunks to avoid runtime latency) and invokes the `Read::read` method. The VFS implementation (e.g., a local filesystem driver) takes ownership of this buffer, fills it with the requested file data, and returns it along with updated file attributes and EOF status.

Inside the system, the following things happen and they use this module: The `Vfs` trait (defined in `vfs::mod`) aggregates the `Read` trait along with other NFS operations (like `Write`, `Create`, etc.). The `handle_forever` loop in the main library (`lib.rs`) uses this trait to process incoming RPCs. By passing the `Buffer` as an argument to `read`, the system decouples the memory allocation policy (handled by the server core) from the data retrieval logic (handled by the VFS backend). This design allows the server to enforce global memory limits and utilize zero-copy techniques (where the network stack reads directly from the buffer filled by the VFS) without requiring the VFS backend to be aware of the server's internal memory management constraints.