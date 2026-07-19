<!-- SPEC_HASH: 0a8aaeead52373671f362026dd834928569ddcf022842d504122d1d2bdffe03c -->
# Module Specification

Module: nfs_mamont::vfs::write
Rust File: src/vfs/write.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`num_derive`**: Used to derive `FromPrimitive` and `ToPrimitive` for the `StableHow` enum. This allows the enum variants to be mapped to and from the integer values used in the NFSv3 wire protocol.
- **`crate::allocator::Buffer`**: Used as a generic bound (`B: Buffer`) for the `Args` struct and the `Write` trait. This abstracts the memory source for the data being written, allowing the VFS layer to operate on buffers managed by the server's custom allocator without knowing the concrete implementation.
- **`crate::consts::nfsv3::NFS3_WRITEVERFSIZE`**: Used to define the size of the `Verifier` struct. This ensures the verifier matches the fixed size required by the NFSv3 protocol for write cookies.
- **`crate::vfs`**: Used to import `WccData` (used in `Success` and `Fail`) and `Error` (used in `Fail`). These types provide the standard mechanism for Weak Cache Consistency and error reporting across the VFS layer.
- **`super::file`**: Used to import `Handle` (used in `Args` and `ArgsPartial`) and `Type` (referenced in documentation). `Handle` identifies the target file, and `Type` is referenced to specify that the target must be a regular file.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface contract for the NFSv3 `WRITE` procedure.
- To specify the data structures required to request a write operation, including the stability requirements (`StableHow`) and the data payload.
- To define the structures for the response, including the count of bytes written, the actual stability achieved, a verifier for cache consistency, and Weak Cache Consistency (WCC) data.
- To provide a mechanism (`ArgsPartial`) to separate the parsing of scalar arguments from the data buffer, facilitating staged request processing.

Inputs:
- **`Args<B>`**: A structure containing the `file::Handle` of the target file, the `offset` (u64) at which to write, the `size` (u32) of the data, the `StableHow` requirement, and the data buffer `B`.
- **`Write::write`**: An asynchronous method invocation taking `Args<B>`.

Outputs:
- **`Result<Success, Fail>`**: The result of the write operation.
    - **`Success`**: Contains `file_wcc` (post-op attributes), `count` (bytes written), `committed` (actual stability), and `verifier`.
    - **`Fail`**: Contains `error` (the specific failure reason) and `wcc_data` (attributes for cache consistency).

Steps:
1. **Argument Specification**: The module defines `Args<B>` to encapsulate all parameters needed for a write. This includes the file handle, offset, size, stability requirement, and the data buffer itself.
2. **Stability Definition**: The `StableHow` enum defines the contract for data persistence:
   - `Unstable`: The server may cache data.
   - `DataSync`: The server must commit data to stable storage.
   - `FileSync`: The server must commit data and metadata to stable storage.
3. **Partial Argument Handling**: The module defines `ArgsPartial`, which mirrors `Args` but excludes the `data` buffer. This allows the caller (likely the RPC layer) to parse and validate the scalar fields before allocating or processing the potentially large data buffer.
4. **Result Construction**: The module defines `Success` and `Fail` structs. `Success` includes a `Verifier` (an opaque byte array of size `NFS3_WRITEVERFSIZE`) which acts as a cookie for the client to verify server state (e.g., detecting reboots) in subsequent `COMMIT` operations. Both result types include `WccData` to allow the client to update its attribute cache.
5. **Trait Declaration**: The `Write<B>` trait declares the `write` method, enforcing that any VFS implementation must accept these arguments and return these results asynchronously.

Edge Cases:
- **Short Writes**: The documentation for `Args` notes that if the data size exceeds the server's `write_max`, the server may write fewer bytes than requested. The `count` field in `Success` reflects the actual number written.
- **Quota Errors**: The documentation for `Write::write` notes that implementations may return `vfs::Error::NoSpace` instead of `vfs::Error::QuotaExceeded` when a user's quota is exceeded.
- **Invalid File Type**: If the `file` handle does not refer to a `file::Type::Regular` file, the operation must return `vfs::Error::InvalidArgument`.

Complexity:
- **Time**: N/A (This module defines interfaces and data structures; it contains no executable logic).
- **Space**: O(1) for the defined structs (excluding the generic buffer `B`).

Determinism:
- **Deterministic**: The module defines static types and interfaces.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
    - **`Handle`**: Used in `Args` and `ArgsPartial` to uniquely identify the file system object to be written to.
    - **`Type`**: Referenced in documentation to constrain the valid targets of the write operation to `Regular` files.

- **From `nfs_mamont::vfs`**:
    - **`WccData`**: Used in `Success` and `Fail` to carry Weak Cache Consistency information. This allows the client to synchronize its cached view of the file's attributes with the server's state before and after the write attempt.
    - **`Error`**: Used in `Fail` to report the specific failure reason (e.g., `NoSpace`, `IO`, `Access`), mapping the VFS operation result to standard NFS status codes.

- **From `nfs_mamont::allocator`**:
    - **`Buffer`**: Used as the generic type `B` for the `data` field in `Args`. This allows the write interface to accept data from the server's memory pool (`Slice`) without requiring ownership transfer or copying, enabling zero-copy (or low-copy) network I/O.

---

## 4. Data Model

Entities:
- **`StableHow`**: An enumeration defining the level of data commitment required by the client.
    - Variants: `Unstable`, `DataSync`, `FileSync`.
- **`Verifier`**: A structure wrapping a fixed-size byte array `[u8; NFS3_WRITEVERFSIZE]`. It acts as an opaque cookie provided by the server to the client.
- **`Success`**: A structure representing a successful write operation.
    - Fields: `file_wcc` (vfs::WccData), `count` (u32), `committed` (StableHow), `verifier` (Verifier).
- **`Fail`**: A structure representing a failed write operation.
    - Fields: `error` (vfs::Error), `wcc_data` (vfs::WccData).
- **`Args<B: Buffer>`**: A structure holding the arguments for the write operation.
    - Fields: `file` (file::Handle), `offset` (u64), `size` (u32), `stable` (StableHow), `data` (B).
- **`ArgsPartial`**: A structure identical to `Args` but without the `data` field.
    - Fields: `file` (file::Handle), `offset` (u64), `size` (u32), `stable` (StableHow).

Relations:
- **Composition**: `Args` and `ArgsPartial` contain `file::Handle` and `StableHow`.
- **Association**: `Success` and `Fail` contain `vfs::WccData`.
- **Association**: `Success` contains `Verifier`.

Global Invariants:
- **Verifier Size**: The `Verifier` struct always contains exactly `NFS3_WRITEVERFSIZE` bytes.
- **Buffer Consistency**: While not enforced by the type system, the `size` field in `Args` is expected to match the length of the `data` buffer.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type imported from the parent `vfs` module, used within the `Fail` struct.

Error Propagation Strategy:
- **Result Wrapping**: The `write` method returns `Result<Success, Fail>`. The `Fail` struct wraps the `vfs::Error` along with `WccData`, ensuring that error responses still provide cache consistency information.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` variant returned (e.g., `IO` might be transient, `Permission` requires user intervention).

Panics:
- **Allowed**: No. This module defines data structures and interfaces; it does not contain logic that panics.

---

## 6. Traits

List which external traits this module implements:
- **`FromPrimitive`**: Implemented for `StableHow` (via `num_derive`).
- **`ToPrimitive`**: Implemented for `StableHow` (via `num_derive`).
- **`Clone`**: Implemented for `StableHow`.
- **`Copy`**: Implemented for `StableHow`.
- **`Eq`**: Implemented for `StableHow`.
- **`PartialEq`**: Implemented for `StableHow`.
- **`Debug`**: Implemented for `StableHow`.

List which traits this module defines:
- **`Write<B: Buffer>`**: The trait defining the asynchronous write operation interface for the VFS.

---

## 7. Overview

This module is used in order to **define the interface and data structures for the NFSv3 WRITE procedure** within the `nfs_mamont` server. The system contains a complex architecture where the network layer (RPC) must communicate with the storage layer (VFS) using a strict contract defined by the NFS protocol. This module serves as the definition of that contract for write operations.

The system requires a way to specify not just *what* data to write, but *how* stable that data must be upon completion (e.g., cached in RAM vs. flushed to disk). The `StableHow` enum defined here captures this requirement. Furthermore, the system must handle the possibility of "unstable" writes where data is cached but not yet committed. To support this, the module defines the `Verifier` struct, which acts as a cookie the client can use later (via a `COMMIT` operation) to verify that the server hasn't rebooted and lost the cached data.

A typical usage scenario of the system involves the RPC layer receiving a WRITE request packet. It parses the scalar fields (file handle, offset, stability) into an `ArgsPartial` struct. It then allocates a buffer from the memory pool (implementing `Buffer`) and fills it with the data payload from the network. It constructs the full `Args` struct and invokes the `write` method on the VFS implementation. The VFS implementation writes the data to the backend storage, respecting the `StableHow` flag. It returns a `Success` struct containing the number of bytes actually written (which might be less than requested), the `committed` status (indicating if the data is actually stable), and a `verifier`. The RPC layer serializes this back to the client.

Inside the system, the following things happen and they use this module:
1.  **Protocol Compliance**: The `Write` trait ensures that any storage backend plugged into the server supports the specific arguments and returns the specific result types required by the NFSv3 specification (RFC 1813).
2.  **Cache Consistency**: By including `WccData` in both `Success` and `Fail`, the module ensures that the client can update its attribute cache regardless of whether the write succeeded or failed, which is critical for maintaining consistency in distributed file systems.
3.  **Memory Abstraction**: By using the generic `Buffer` trait for the `data` field in `Args`, the module decouples the VFS logic from the specific memory management strategy of the server, allowing the use of high-performance pooled memory.

Without this module, the definition of the write operation would be scattered across the RPC parsing logic and the storage backend, leading to potential mismatches in protocol handling and a lack of type safety for stability guarantees. This module centralizes these definitions, providing a clear contract for both the network layer and the storage implementors.