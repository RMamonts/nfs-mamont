<!-- SPEC_HASH: e26eb69346c9b4024ae82ecb51ea5bc9044f2881bd72340ba2e6595c30e501fd -->
# Module Specification

Module: nfs_mamont::vfs::access
Rust File: src/vfs/access.rs

---

## 1. Dependencies

From the source code and context, for each dependency, write down its purpose — why it is used in this module.

- **`super::{file, Error}`**: Used to import the `file::Handle` type to identify the target file system object and the `file::Attr` type to return object metadata. It also imports the `Error` enum to define the failure variant of the access check result.
- **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` attribute to automatically generate an object-safe version of the `Access` trait that implements `Send`. This is necessary to allow the trait to be used as an async trait object in a multi-threaded RPC server context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `ACCESS` procedure, which allows a client to determine the access rights a user has regarding a specific file system object.
- To provide a type-safe representation of the NFSv3 access permission bitmask (`Mask`), including constants for specific rights (READ, LOOKUP, etc.) and logic to sanitize wire-formatted input.
- To structure the arguments and results of the access check operation, including the optional return of file attributes to optimize client-side cache updates (avoiding a subsequent `GETATTR` call).

Inputs:
- **`Args`**: A structure containing:
  - `file`: A `file::Handle` identifying the object to check.
  - `mask`: A `Mask` specifying the permissions to check.
- **`u32` (raw)**: Used in `Mask::from_wire` to construct a mask from network protocol data.

Outputs:
- **`Result<Success, Fail>`**: The result of the access check.
  - `Success`: Contains the `access` mask (bits that are allowed) and optionally the `object_attr`.
  - `Fail`: Contains the `error` (an `Error` enum variant) and optionally the `object_attr`.
- **`Mask`**: A newtype wrapper around `u32` representing access rights.

Steps:
1. **Mask Sanitization**: When `Mask::from_wire` is called with a raw `u32`, it applies a bitmask `Self::ALL` to ignore any undefined or reserved bits set in the input, ensuring the internal state only contains valid protocol flags.
2. **Access Check Invocation**: The `Access::access` method is called asynchronously with `Args`. The implementation (defined elsewhere) checks the requested permissions against the file system's ACLs or mode bits for the user context associated with the request.
3. **Result Construction**:
   - If the check is logically successful (even if access is denied), the implementation returns `Ok(Success)`. The `Success` struct contains a `Mask` indicating which of the requested rights are actually granted.
   - If an error occurs (e.g., stale file handle, server I/O error), the implementation returns `Err(Fail)`.
4. **Attribute Attachment**: Both `Success` and `Fail` structs allow attaching `Option<file::Attr>`. This is used to return the current attributes of the object, allowing the client to update its cache regardless of whether the access check itself succeeded or failed.

Edge Cases:
- **Advisory Nature**: The documentation explicitly states that the results are advisory. A successful check does not guarantee future access, as permissions can be revoked immediately after the check.
- **Undefined Bits**: `Mask::from_wire` silently drops bits not present in `ALL` (e.g., bit 0x0040 or higher). This prevents internal state corruption from malformed or future-compatible protocol messages.
- **Attribute Availability**: The `object_attr` in `Success` and `Fail` is optional. The server may choose not to return attributes if they are expensive to retrieve or if the client did not request them (though the NFSv3 protocol usually recommends returning them if available).

Complexity:
- **Time**:
  - `Mask::from_wire`: O(1).
  - `Mask::contains`: O(1).
  - `Access::access`: Depends on the implementation (file system lookup, permission check).
- **Space**:
  - `Mask`: O(1) (size of `u32`).
  - `Args`, `Success`, `Fail`: O(1) stack space, plus the size of the contained `Handle` or `Attr`.

Determinism:
- **Deterministic**: The logic for masking bits is deterministic. The result of `Access::access` depends on the state of the underlying file system and the user credentials, which are external inputs, but the interface itself enforces a deterministic contract for data representation.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
  - **`Handle`**: Used in `Args` to uniquely identify the file system object being queried. The `Access` operation relies on this opaque identifier to locate the inode or metadata structure without requiring a full path.
  - **`Attr`**: Used in `Success` and `Fail` to return the file's attributes. This leverages the attribute structure defined in the `file` module to provide metadata (size, mode, times) back to the client, facilitating cache consistency.

- **From `nfs_mamont::vfs`**:
  - **`Error`**: Used in `Fail` to report specific protocol or server errors. This ensures that the `Access` interface integrates seamlessly with the centralized error handling of the VFS layer (e.g., returning `StaleFile` if the handle is invalid).

---

## 4. Data Model

Entities:
- **`Args`**: Arguments for the access check.
  - `file`: `file::Handle` (Target object).
  - `mask`: `Mask` (Requested permissions).
- **`Success`**: Successful result of the access check.
  - `object_attr`: `Option<file::Attr>` (Current attributes).
  - `access`: `Mask` (Granted permissions).
- **`Fail`**: Failure result of the access check.
  - `error`: `Error` (The reason for failure).
  - `object_attr`: `Option<file::Attr>` (Current attributes, if available).
- **`Mask`**: Bitmask for access rights.
  - `0`: `u32` (Internal storage).
  - Constants: `READ`, `LOOKUP`, `MODIFY`, `EXTEND`, `DELETE`, `EXECUTE`, `ALL`.

Relations:
- **Composition**: `Args` composes `file::Handle` and `Mask`.
- **Composition**: `Success` and `Fail` compose `Option<file::Attr>` and `Mask` (for `Success`) or `Error` (for `Fail`).

Global Invariants:
- **Mask Validity**: Any instance of `Mask` created via `from_wire` guarantees that `self.0 & !Self::ALL == 0`. This ensures no undefined flags are set.
- **Advisory Result**: The `access` field in `Success` represents the permissions *available* at the moment of the check, not a guarantee of future access.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type wrapped in the `Fail` struct. This enum covers standard NFSv3 errors (like `Access`, `IO`, `StaleFile`) and implementation-specific errors.

Error Propagation Strategy:
- **Result Wrapper**: The `access` method returns `Result<Success, Fail>`. If the operation fails (e.g., the file handle is invalid or an I/O error occurs), the implementor returns `Err(Fail { error, .. })`.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` variant. For example, `JUKEBOX` implies the client should retry, while `StaleFile` requires the client to perform a new lookup.

Panics:
- **Allowed**: No. The interface is designed for data transfer and error reporting via `Result`.

---

## 6. Traits

List which external traits this module implements:
- **`Send`**: The `#[trait_variant::make(Send)]` attribute generates a trait named `Access` (or an object-safe version of it) that implements `Send`, allowing the trait object to be passed between threads. The original trait definition is modified by this macro.

List which traits this module defines:
- **`Access`**: The core trait defining the `access` asynchronous method. This trait is intended to be implemented by the VFS backend.

---

## 7. Overview

This module is used in order to **implement the NFSv3 ACCESS procedure**, which allows clients to verify user permissions on a file system object before attempting operations that might fail due to lack of rights. The system contains a complex architecture where the server must translate high-level file system concepts into specific protocol responses. This module defines the contract for that specific translation.

A typical usage scenario of the system involves a client wishing to write to a file but wanting to verify write permission first to avoid a wasted RPC attempt. The client sends an `ACCESS` request with the `MODIFY` bit set. The RPC layer receives this request, extracts the file handle and the mask, and invokes the `access` method on the `Vfs` trait object. The underlying storage backend checks the file's Access Control List (ACL) or mode bits against the user's credentials. It returns a `Success` struct containing a `Mask` where the `MODIFY` bit is set (or unset) and the current file attributes. The RPC layer serializes this back to the client.

Inside the system, the following things happen and they use this module:
1.  **Permission Checking**: The `Access` trait is the boundary where the abstract concept of "user permissions" is materialized into the specific bitmask format defined by the NFSv3 protocol.
2.  **Cache Optimization**: By including `Option<file::Attr>` in both `Success` and `Fail`, this module supports the NFS optimization of returning attributes along with the operation result. This allows the client to update its attribute cache without issuing a separate `GETATTR` call, reducing network latency.
3.  **Protocol Safety**: The `Mask` type ensures that only valid, protocol-defined bits are processed. The `from_wire` method acts as a sanitizer, preventing undefined bits from entering the system logic, which is crucial for robustness when handling untrusted network input.

Without this module, the VFS layer would lack a standardized way to report access checks, forcing the RPC layer to rely on generic error codes or ad-hoc structures. This would make it difficult to provide the specific "access mask" feedback required by the NFSv3 specification, potentially leading to inefficient client behavior (e.g., clients blindly attempting operations they know will fail). This module encapsulates the specific semantics of the `ACCESS` procedure, ensuring the server adheres to the protocol's advisory permission model.