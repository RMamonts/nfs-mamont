<!-- SPEC_HASH: 7a442ffdfb84cefce07313b3c3ee85ea577b30d81d04a22bbc3ff60803cc05e6 -->
# Module Specification

Module: nfs_mamont::vfs::read
Rust File: src/vfs/read.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::allocator::Buffer`**: Used as a generic bound `B` for the `Read` trait and `Success` struct. It represents the memory region allocated by the server that the implementation must fill with file data. This decouples the memory allocation strategy (pool-based) from the file reading logic.
- **`crate::vfs`**: Used to import the `Error` enum, which is included in the `Fail` struct to report specific failure conditions (e.g., `IO`, `InvalidArgument`) to the caller.
- **`super::file`**: Used to import `Handle` (to identify the file), `Attr` (to return post-operation metadata), and `Type` (to enforce constraints on the file type). These types are essential for constructing the arguments (`Args`) and results (`Success`, `Fail`) of the read operation.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `READ` procedure.
- To enforce a "server-allocated buffer" pattern where the caller provides the memory (`data: B`) to be filled, ensuring the server controls memory allocation and limits.
- To guarantee that file read operations return post-operation attributes (`file_attr`) to support Weak Cache Consistency (WCC) without requiring a separate `GETATTR` call.

Inputs:
- **`args: Args`**: Specifies the target file (`file`), the starting byte offset (`offset`), and the number of bytes to read (`count`).
- **`data: B`**: A buffer instance implementing the `Buffer` trait. This buffer is pre-allocated by the server and passed to the implementation to be filled.

Outputs:
- **`Result<Success<B>, Fail>`**:
    - **`Success<B>`**: Contains the filled buffer (`data`) and metadata (`head`).
    - **`Fail`**: Contains the error (`error`) and optional metadata (`file_attr`).

Steps:
1. **Argument Validation**: The implementation checks if `args.file` refers to a file system object of type `file::Type::Regular`. If not, it returns `Fail` with `vfs::Error::InvalidArgument`.
2. **Offset Check**: The implementation checks if `args.offset` is greater than or equal to the current size of the file. If so, it returns `Success` with `count` set to `0` and `eof` set to `true`.
3. **Data Transfer**: The implementation reads up to `args.count` bytes from the file starting at `args.offset`. The data is written directly into the provided `data` buffer.
4. **EOF Determination**: The implementation determines if the read operation reached the end of the file.
5. **Attribute Retrieval**: The implementation retrieves the current attributes of the file (`file::Attr`) after the read operation.
6. **Result Construction**:
    - On success, it constructs `SuccessPartial` with the retrieved attributes, the actual number of bytes read (`count`), and the `eof` flag. It wraps this and the filled buffer into `Success`.
    - On failure, it constructs `Fail` with the specific `vfs::Error` and the post-operation attributes (if available).

Edge Cases:
- **Zero Count**: If `args.count` is `0`, the operation succeeds immediately, returning `0` bytes.
- **Short Reads**: If `args.count` exceeds the server's `read_max` (defined in `fs_info`), the implementation may return fewer bytes than requested.
- **EOF Handling**: Reading exactly to the end of the file sets `eof` to `true`. Reading past the end returns `count = 0` and `eof = true`.

Complexity:
- **Time**: Dependent on the underlying storage implementation (disk I/O latency).
- **Space**: O(1) stack space for the structs. The heap space is determined by the size of the buffer `B` passed in.

Determinism:
- **Deterministic**: The behavior is strictly defined by the NFSv3 protocol and the input arguments. The specific error codes returned depend on the state of the file system.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::allocator`**:
    - **Buffer Trait**: The `Read` trait relies on the `Buffer` trait to abstract the destination of the file data. The critical mechanism here is the transfer of ownership: the `read` method takes ownership of `data: B` and returns it inside `Success<B>`. This ensures that the buffer's lifecycle is tied to the result, allowing the allocator to reclaim the memory (via `Drop`) once the RPC response is sent.

- **From `nfs_mamont::vfs::file`**:
    - **Handle**: Used to uniquely identify the file to be read. The implementation uses this handle to locate the file system object.
    - **Attr**: The module requires the implementation to return `file::Attr` in both success and failure cases. This is a specific requirement of the NFSv3 protocol to allow clients to update their attribute caches efficiently.
    - **Type**: The module explicitly constrains the operation to `file::Type::Regular`. This mechanism prevents reading from directories or special files, which are handled by different NFS procedures.

---

## 4. Data Model

Entities:
- **`Args`**: Arguments for the read operation.
    - `file`: `file::Handle` (Identifier of the file).
    - `offset`: `u64` (Starting position).
    - `count`: `u32` (Number of bytes to read).
- **`Success<B>`**: Successful result wrapper.
    - `head`: `SuccessPartial` (Metadata and status).
    - `data`: `B` (The filled buffer).
- **`SuccessPartial`**: Metadata component of the success result.
    - `file_attr`: `Option<file::Attr>` (Post-operation attributes).
    - `count`: `u32` (Actual bytes read).
    - `eof`: `bool` (End-of-file flag).
- **`Fail`**: Failure result wrapper.
    - `error`: `vfs::Error` (The error code).
    - `file_attr`: `Option<file::Attr>` (Post-operation attributes, if available).

Relations:
- **Composition**: `Success` contains `SuccessPartial` and the generic `Buffer`.
- **Association**: `Args` references a `file::Handle`. `SuccessPartial` and `Fail` reference `file::Attr`.

Global Invariants:
- **Count Consistency**: `SuccessPartial.count` must be less than or equal to `Args.count`.
- **Buffer Validity**: The `data` field in `Success` must contain exactly `SuccessPartial.count` bytes of valid file data starting from `Args.offset`.
- **EOF Semantics**: If `SuccessPartial.eof` is `true`, either `SuccessPartial.count` is `0` (offset past end) or the read ended exactly at the last byte of the file.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error enumeration defined in the parent `vfs` module. Specific errors mentioned in the documentation include `vfs::Error::InvalidArgument` (for non-regular files).

Error Propagation Strategy:
- **Result Wrapper**: Errors are propagated via the `Err` variant of `Result<Success<B>, Fail>`. The `Fail` struct wraps the `vfs::Error` and includes optional attributes to allow for cache recovery on the client side.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` returned. For example, `IO` errors might be transient (retryable), while `InvalidArgument` or `StaleFile` indicate persistent issues with the request or file handle state.

Panics:
- **Allowed**: No. The interface defines a contract based on `Result`; panics are not part of the expected error handling mechanism for this trait.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`Read<B: Buffer>`**: Defines the asynchronous read operation.
    - **`async fn read(&self, args: Args, data: B) -> Result<Success<B>, Fail>`**: Reads data from a file into the provided buffer. The trait is generated with `#[trait_variant::make(Send)]`, ensuring the resulting trait object is `Send`.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than this level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than this level.

This module is used in order to **define the contract for reading file data in the NFSv3 server implementation**, specifically enforcing the separation of memory allocation from I/O execution. The system contains a high-performance network server that must strictly control memory usage to prevent denial-of-service attacks and ensure predictable latency. By defining the `Read` trait to accept a pre-allocated `Buffer` (`data: B`), this module ensures that the storage backend (the VFS implementation) cannot perform arbitrary heap allocations. Instead, it must fill the memory provided by the server's custom allocator (`nfs_mamont::allocator`).

A typical usage scenario of the system involves the RPC layer receiving a `READ` request from a client. The RPC layer allocates a buffer of the requested size from the memory pool. It then invokes the `read` method on the VFS implementation, passing the request arguments (`Args`) and the allocated buffer. The VFS implementation reads the data from the disk directly into this buffer and returns it wrapped in `Success`, along with the file attributes. The RPC layer then sends the buffer contents and attributes back to the client. Once the response is sent, the `Success` struct (and the buffer inside it) is dropped, automatically returning the memory to the pool.

Inside the system, the following things happen and they use this module:
1.  **Memory Control**: The `Read` trait is the boundary where the server's memory policy (enforced by the allocator) meets the storage logic. By requiring the buffer as an input, the system guarantees that memory for I/O is always sourced from the pre-allocated pool, avoiding fragmentation and heap overhead.
2.  **Protocol Compliance**: The module structures the return types (`Success`, `Fail`) to match the NFSv3 wire format requirements. Specifically, the inclusion of `file_attr` in both success and failure cases supports the Weak Cache Consistency (WCC) model, allowing clients to validate their cached metadata without extra network calls.
3.  **Type Safety**: The use of `Args` and `file::Handle` ensures that the VFS implementation receives strongly typed, validated inputs, reducing the risk of errors in interpreting file offsets or handles.

The critical aspect of this module is the **inversion of control for memory allocation**. Unlike standard Rust I/O traits where the callee might return a `Vec<u8>`, here the caller provides the storage. This design is essential for the `nfs_mamont` server's architecture to maintain high performance and stability under load.