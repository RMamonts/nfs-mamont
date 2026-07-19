<!-- SPEC_HASH: e26eb69346c9b4024ae82ecb51ea5bc9044f2881bd72340ba2e6595c30e501fd -->
# Module Specification

Module: nfs_mamont::vfs::access
Rust File: src/vfs/access.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **super::file**: Used to import `file::Handle` and `file::Attr`. `Handle` is required to identify the target file system object within the arguments, and `Attr` is required to populate the optional object attributes in both success and failure results, adhering to the NFSv3 protocol requirement to return attributes alongside access checks.
- **super::Error**: Used to import the `Error` enum. This type is used within the `Fail` struct to specify the precise reason for the access check failure (e.g., `StaleFile`, `Access`, `IO`).
- **trait_variant::make**: Used as a procedural macro attribute on the `Access` trait. It transforms the async trait into an object-safe trait that also implements `Send`, allowing the trait to be used as a trait object in multi-threaded asynchronous contexts (e.g., `dyn Access + Send`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `ACCESS` procedure, abstracting the logic of checking user permissions on file system objects.
- To provide a type-safe representation of the NFSv3 access bitmask (`Mask`), ensuring that only valid bits are considered when processing requests from the network.
- To structure the results of the access check such that they always include the calculated access rights and optionally include the file attributes, facilitating client-side cache updates.

Inputs:
- `Args`: A struct containing a `file::Handle` (identifying the object) and a `Mask` (the bits requested by the client).
- Raw `u32`: Used to construct a `Mask` via `from_wire`.

Outputs:
- `Result<Success, Fail>`:
  - `Success`: Contains the `Mask` of rights granted to the user and an `Option<file::Attr>` containing the object's attributes.
  - `Fail`: Contains the `Error` that occurred and an `Option<file::Attr>` (which may be present if the object was found but access was denied).

Steps:
1. **Mask Sanitization**: When a `Mask` is created using `from_wire`, the input `u32` is bitwise-ANDed with `Mask::ALL` (0x003F). This ensures that any undefined or protocol-violating bits are ignored, strictly adhering to the NFSv3 specification.
2. **Access Check Invocation**: The `Access::access` method is called with `Args`. The implementation (not defined here) is expected to resolve the `file::Handle` to a concrete file system object and evaluate the requested permissions against the user's credentials.
3. **Result Construction**:
   - If the check succeeds, a `Success` struct is returned. The `access` field contains a `Mask` representing the subset of requested rights that are actually granted.
   - If the check fails (e.g., the handle is stale or an I/O error occurs), a `Fail` struct is returned containing the specific `Error`.
4. **Attribute Attachment**: Both `Success` and `Fail` structs carry an `Option<file::Attr>`. The implementation determines whether to populate this field based on the ability to retrieve attributes without incurring significant additional cost or error.

Edge Cases:
- **Undefined Bits**: If `from_wire` receives a `u32` with bits set outside of `READ`, `LOOKUP`, `MODIFY`, `EXTEND`, `DELETE`, or `EXECUTE`, those bits are silently dropped.
- **Advisory Nature**: The documentation explicitly states that a successful `Ok` result does not guarantee future access, as permissions can be revoked asynchronously.
- **Attribute Availability**: The `object_attr` in `Success` and `Fail` is optional. A client must handle the case where attributes are not returned even if the access check itself succeeded or failed for a reason other than "file not found".

Complexity:
- Time: O(1) for `Mask` operations (bitwise checks). The complexity of `Access::access` depends on the underlying VFS implementation but is generally expected to be O(1) or O(log N) relative to the file system depth.
- Space: O(1) for `Mask`, `Args`, `Success`, and `Fail`. `file::Attr` is a fixed-size struct.

Determinism:
- Deterministic. Given the same file handle, user credentials (implicit in the request context), and input mask, the result must be consistent.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
  - **Handle**: Acts as the opaque identifier for the file system object. The `Access` trait relies on this to locate the target without knowing the internal storage structure.
  - **Attr**: Provides the metadata structure. The `access` interface returns this to allow the NFS client to update its attribute cache, reducing the need for subsequent `GETATTR` calls.
- **From `nfs_mamont::vfs`**:
  - **Error**: Enumerates the specific failure modes (e.g., `StaleFile`, `Access`, `IO`). The `Fail` struct wraps this enum to inform the client exactly why the access check could not be completed.

---

## 4. Data Model

Entities:
- **Args**: A parameter bundle containing `file` (Handle) and `mask` (Mask).
- **Success**: A result bundle containing `object_attr` (Option<Attr>) and `access` (Mask).
- **Fail**: A result bundle containing `error` (Error) and `object_attr` (Option<Attr>).
- **Mask**: A wrapper around `u32` representing a set of access rights.

Relations:
- **Args** → **Mask**: Contains.
- **Args** → **Handle**: Contains.
- **Success** → **Mask**: Contains (the granted rights).
- **Success** → **Attr**: Optionally contains.
- **Fail** → **Error**: Contains.
- **Fail** → **Attr**: Optionally contains.

Global Invariants:
- **Mask Invariants**: The integer value wrapped by `Mask` will never have bits set outside the range defined by `Mask::ALL` (0x003F) if instantiated via `from_wire`.
- **Result Invariants**: In a `Fail` result, if `object_attr` is `Some`, it implies the file handle was valid enough to retrieve attributes, even if the access check itself failed (e.g., due to permission denied). If the error is `StaleFile`, `object_attr` is typically `None`.

## 5. Error Model

Error Types:
- `vfs::Error` (imported as `Error`): An enum covering NFSv3 specific errors (e.g., `Permission`, `NoEntry`, `IO`, `StaleFile`).

Error Propagation Strategy:
- The `access` method returns a `Result<Success, Fail>`. Instead of a simple `Result<T, E>`, the error side is a struct `Fail` that wraps the `Error` enum. This allows the method to return an error while still potentially providing file attributes.

Recoverability:
- Recoverable. The `Fail` struct provides the specific error code, allowing the NFS server to map it back to the correct NFS status code and send it to the client.

Panics:
- Allowed: No
- Conditions: The code consists of struct definitions and a trait definition. There are no explicit panic points. The `Mask` operations are simple bitwise checks on primitive types.

---

## 6. Traits

List which external traits this module implements:
- **std::fmt::Debug**: Implemented for `Mask`.
- **std::marker::Copy**: Implemented for `Mask`.
- **std::clone::Clone**: Implemented for `Mask`.
- **std::cmp::PartialEq**: Implemented for `Mask`.
- **std::cmp::Eq**: Implemented for `Mask`.
- **std::marker::Send**: Implemented for `Access` (via `#[trait_variant::make(Send)]`).

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the contract for permission verification within the NFSv3 server implementation. The system contains a Virtual File System (VFS) layer that abstracts the complexities of various underlying storage backends (e.g., local ext4, network storage) and presents a unified interface to the NFS protocol handler. A typical usage scenario of the system involves an NFS client sending an `ACCESS` request to determine if it can read or write a file before attempting the actual operation, thereby optimizing network usage by avoiding failed attempts. The system uses the `Access` trait defined in this module to decouple the protocol logic (which understands bitmasks and NFS status codes) from the storage logic (which understands Unix permissions, ACLs, or user IDs).

Inside the system, the following things happen and they use this module: The NFS request handler parses the incoming wire format into the `Args` struct (containing the file handle and the requested access mask). It then invokes the `access` method on the VFS backend implementation. The backend uses the `file::Handle` to locate the inode or metadata, checks the effective permissions of the user context associated with the RPC request, and calculates the effective `Mask`. It returns a `Success` struct containing this mask and the current `file::Attr`. The handler then serializes this result back into the NFSv3 response format. The `Mask` struct ensures that only valid protocol bits are ever considered, sanitizing the input from the network. Without this module, the VFS would lack a standardized way to report access rights, leading to inconsistencies between different storage backend implementations and potential security risks if raw permission bits were exposed without proper abstraction.