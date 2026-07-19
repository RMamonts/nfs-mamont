<!-- SPEC_HASH: 6c9d15284c44f32dad6d604849d60b9e23848aab41bd07c2266c48917748f626 -->
# Module Specification

Module: nfs_mamont::vfs::get_attr
Rust File: src/vfs/get_attr.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `vfs::Error` enum. This type is wrapped in the `Fail` struct to provide standardized error codes (e.g., `StaleFile`, `IO`) that map to NFSv3 status codes.
- **`super::file`**: Used to import `file::Attr` and `file::Handle`. `file::Handle` is used in `Args` to identify the target object, and `file::Attr` is used in `Success` to return the object's metadata.
- **`trait_variant`**: Used to generate a `Send` version of the `GetAttr` trait via the `#[trait_variant::make(Send)]` attribute. This allows the trait to be used as an object in asynchronous contexts that require thread safety (e.g., `dyn GetAttr + Send`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `GETATTR` procedure.
- To abstract the retrieval of file system object attributes (metadata) based on a file handle.
- To provide a strongly typed contract (`Args`, `Success`, `Fail`) that separates the input (handle) from the output (attributes or error).

Inputs:
- **`args: Args`**: A structure containing a `file::Handle`, which uniquely identifies the file system object (file, directory, etc.) whose attributes are being requested.

Outputs:
- **`Result<Success, Fail>`**:
 - **`Ok(Success)`**: Contains a `file::Attr` struct with the object's metadata (type, mode, size, timestamps, etc.).
 - **`Err(Fail)`**: Contains a `vfs::Error` indicating why the operation failed (e.g., the handle is stale, permission denied, or an I/O error occurred).

Steps:
1. The caller invokes the `get_attr` method on an implementation of the `GetAttr` trait, passing the `Args` struct containing the target `file::Handle`.
2. The implementation (the backend) attempts to resolve the `file::Handle` to an internal file system object.
3. If the object is found and accessible, the implementation constructs a `file::Attr` struct representing the current state of the object and returns it wrapped in `Success`.
4. If the object cannot be found (e.g., stale handle), access is denied, or an I/O error occurs, the implementation returns a `vfs::Error` wrapped in `Fail`.

Edge Cases:
- **Stale Handle**: If the `file::Handle` refers to an object that no longer exists or has been invalidated, the implementation must return `vfs::Error::StaleFile`.
- **Permission Denied**: If the caller lacks permission to read the attributes, the implementation must return `vfs::Error::Permission` or `vfs::Error::Access`.

Complexity:
- **Time**: Dependent on the backend implementation. Typically O(1) for in-memory lookups or O(log N) for indexed filesystem lookups.
- **Space**: O(1) for the arguments and return structures (stack allocation), though the `file::Attr` struct itself contains multiple fields.

Determinism:
- **Deterministic**: Given a specific file system state and a valid handle, the operation must return the same attributes. Error conditions are also deterministic based on the state of the file system.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: Used as the input key in `Args`. It provides the fixed-size, opaque identifier required by the NFS protocol to reference files without using path strings.
 - **`Attr`**: Used as the payload in `Success`. It provides the comprehensive metadata structure (including `Type`, `Time`, `Device`, etc.) that the NFSv3 protocol requires in a `GETATTR` response.

- **From `nfs_mamont::vfs`**:
 - **`Error` Enum**: Used in `Fail` to classify failures. This ensures that errors returned by `GetAttr` are consistent with the broader VFS error model and can be correctly translated to NFS status codes (e.g., mapping `Error::StaleFile` to `NFS3ERR_STALE`).

---

## 4. Data Model

Entities:
- **`Args`**: Input arguments for the operation.
 - Fields: `file: file::Handle`.
- **`Success`**: Successful result of the operation.
 - Fields: `object: file::Attr`.
- **`Fail`**: Failed result of the operation.
 - Fields: `error: vfs::Error`.

Relations:
- **`Args` → `file::Handle` (Composition)**: `Args` owns the handle to be looked up.
- **`Success` → `file::Attr` (Composition)**: `Success` owns the retrieved attributes.
- **`Fail` → `vfs::Error` (Composition)**: `Fail` owns the error code.

Global Invariants:
- The `file::Handle` inside `Args` must be exactly `NFS3_FHSIZE` bytes (enforced by the `file::Handle` type).
- The `file::Attr` inside `Success` must contain valid metadata consistent with the file system state at the moment of the call.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type wrapped in the `Fail` struct. This enum covers standard NFS errors (e.g., `IO`, `StaleFile`, `Access`).

Error Propagation Strategy:
- **Result Type**: The method returns `Result<Success, Fail>`. The `Fail` struct explicitly wraps the `vfs::Error`, separating the error from the success payload.

Recoverability:
- **Dependent on Error**:
 - `vfs::Error::IO`: Potentially recoverable if the transient condition clears.
 - `vfs::Error::StaleFile`: Not recoverable for the specific handle; the client must perform a new lookup to obtain a valid handle.
 - `vfs::Error::Access` / `vfs::Error::Permission`: Not recoverable without changing user permissions.

Panics:
- **Allowed**: No. The trait definition implies a fallible operation via `Result`, so implementations should return errors rather than panic.

---

## 6. Traits

List which external traits this module implements:
- **`Send`**: The `GetAttr` trait is transformed into a `Send` trait object via `trait_variant`, allowing it to be passed across thread boundaries in an async runtime.

List which traits this module defines:
- **`GetAttr`**: The core trait defining the `get_attr` asynchronous method.

---

## 7. Overview

This module is used in order to **define the interface for retrieving file metadata** within the `nfs_mamont` NFSv3 server. The system contains a complex architecture where the Virtual File System (VFS) is abstracted behind a set of traits, allowing different storage backends to be plugged in. This module specifically handles the `GETATTR` NFS procedure, which is one of the most fundamental operations in the protocol, used by clients to verify file existence, check permissions, and synchronize caches.

A typical usage scenario of the system involves an NFS client sending a `GETATTR` request for a specific file handle. The RPC layer of the server receives this request, extracts the handle, and constructs the `Args` struct. It then invokes the `get_attr` method on the VFS implementation (which implements the `GetAttr` trait). The backend looks up the handle, retrieves the current metadata (size, mode, times, etc.), packages it into a `file::Attr`, and returns it inside the `Success` struct. If the handle is invalid or an error occurs, the backend returns a `Fail` struct with the appropriate `vfs::Error`. The RPC layer then serializes this result back to the client.

Inside the system, the following things happen and they use this module:
1.  **Protocol Compliance**: The `vfs::mod` module aggregates the `GetAttr` trait into the main `Vfs` super-trait. This ensures that any storage backend claiming to be a VFS must support attribute retrieval.
2.  **Type Safety**: By using `Args` and `Success` structs, the module enforces a strict boundary between the raw bytes of the file handle and the interpreted metadata. This prevents the RPC layer from accidentally passing invalid data types to the storage backend.
3.  **Error Standardization**: The use of `vfs::Error` inside `Fail` ensures that attribute retrieval failures are reported using the same error codes as other VFS operations (like `READ` or `WRITE`), maintaining consistency across the server.

Without this module, the VFS interface would lack a dedicated definition for attribute retrieval, forcing the server to rely on ad-hoc methods or combine this logic with other operations (like `LOOKUP`), which would violate the separation of concerns and make the codebase harder to maintain and less compliant with the NFSv3 specification.