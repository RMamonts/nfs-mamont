<!-- SPEC_HASH: d13292a954bf9389d5ef225dde67df630a448e935c57cb5a34120006f277ddad -->
# Module Specification

Module: nfs_mamont::vfs::set_attr
Rust File: src/vfs/set_attr.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::vfs**: Used to import `vfs::WccData` and `vfs::Error`. `WccData` is required in the return types (`Success` and `Fail`) to provide Weak Cache Consistency information to the client. `vfs::Error` is used to report specific failure conditions such as synchronization mismatches or invalid arguments.
- **super::file**: Used to import `file::Handle` and `file::Time`. `file::Handle` is used within `Args` to identify the target file system object. `file::Time` is used within `Guard` and `SetTime` to represent and verify timestamps.
- **trait_variant::make**: Used to transform the `SetAttr` trait into a `Send` trait object. This is necessary to allow the implementation of the VFS to be passed across thread boundaries in an asynchronous context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for modifying file system object attributes (metadata) according to the NFSv3 `SETATTR` procedure.
- To provide a mechanism for optimistic locking using `Guard` to prevent lost updates when multiple clients modify attributes concurrently.
- To support flexible timestamp updates (server time, client time, or no change) via the `SetTime` strategy.
- To define the contract for file size manipulation (truncation, extension) and its side effects on modification time (`mtime`).

Inputs:
- `Args`: A structure containing:
  - `file`: A `file::Handle` identifying the object to modify.
  - `new_attr`: A `NewAttr` structure specifying which attributes to change (mode, uid, gid, size, atime, mtime).
  - `guard`: An optional `Guard` containing the expected `ctime` of the object before modification.

Outputs:
- `Result<Success, Fail>`:
  - `Success`: Contains `vfs::WccData` reflecting the state of the object before and after the operation.
  - `Fail`: Contains `vfs::Error` describing the failure and `vfs::WccData` reflecting the state (usually the pre-operation state if the operation failed).

Steps:
1. **Guard Verification**: If `Args::guard` is `Some`, the implementation must compare the `ctime` value in the guard with the current `ctime` of the file system object.
   - If they differ, the implementation must not modify any attributes and must return `Err(Fail)` where `error` is `vfs::Error::NotSync`.
2. **Attribute Application**: The implementation iterates over the fields in `Args::new_attr`.
   - For each field that is `Some` (e.g., `mode`, `uid`, `gid`), the corresponding attribute on the object is updated.
3. **Size Modification**: If `Args::new_attr.size` is `Some`:
   - If the value is `0`, the file is truncated to zero length.
   - If the value is less than the current size, data beyond the new size is discarded.
   - If the value is greater than the current size, the file is extended. The implementation may use sparse blocks (holes) or actual zero bytes.
   - **Side Effect**: The `mtime` (modification time) of the object must be updated as a result of the size change.
4. **Timestamp Update**: The `atime` and `mtime` fields are updated based on the `SetTime` strategy:
   - `DontChange`: The timestamp is not modified.
   - `ToServer`: The timestamp is set to the server's current time.
   - `ToClient(time)`: The timestamp is set to the value provided by the client.
5. **WCC Data Construction**: The implementation captures the attributes of the object before the operation (if available) and after the operation to populate `vfs::WccData`.
6. **Result Return**: The method returns `Ok(Success)` if the operation proceeds, or `Err(Fail)` if an error occurs (including the `NotSync` guard failure).

Edge Cases:
- **Partial Failure**: The specification explicitly states that `set_attr` is not guaranteed to be atomic. If the method returns `Err(Fail)`, some attributes may have already been modified.
- **UID/GID Limits**: If the implementation cannot store the provided `uid` or `gid` (e.g., due to internal representation limits), it must return `vfs::Error::InvalidArgument`.
- **32-bit Offset Limits**: If the implementation only supports 32-bit offsets and the requested size exceeds `u32::MAX`, it must return `vfs::Error::InvalidArgument` (or potentially `vfs::Error::FileTooLarge` depending on interpretation, though the docstring specifies `InvalidArgument` for this specific case).

Complexity:
- Time: Dependent on the underlying file system implementation. The interface itself involves simple data structure access.
- Space: O(1) for the arguments and return structures.

Determinism:
- Deterministic (assuming the underlying file system state is stable).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs` (Assumption based on facts)**: `vfs::WccData` is a struct containing `before: Option<file::WccAttr>` and `after: Option<file::Attr>`. This mechanism is critical for the `Success` and `Fail` return types to allow the client to verify its cache state. `vfs::Error` is an enum defining specific error codes like `NotSync` (value 10002) and `InvalidArgument` (value 22), which are returned by the `set_attr` logic.
- **From `crate::vfs::file`**: `file::Handle` is a fixed-size byte array `[u8; NFS3_FHSIZE]` acting as an opaque identifier for the file. `file::Time` is a struct with `seconds` and `nanos` fields, used for precise timestamp comparisons in the `Guard` and updates in `SetTime`.

---

## 4. Data Model

Entities:
- **Guard**: A struct containing `pub ctime: file::Time`. It acts as a verification token to ensure the object has not been modified by another client since the last check.
- **SetTime**: An enum defining the strategy for updating timestamps.
  - `DontChange`: Retain the current timestamp.
  - `ToServer`: Set timestamp to the server's current time.
  - `ToClient(file::Time)`: Set timestamp to a specific value provided by the client.
- **NewAttr**: A struct collecting requested attribute changes. All fields are optional to allow partial updates.
  - `pub mode: Option<u32>`
  - `pub uid: Option<u32>`
  - `pub gid: Option<u32>`
  - `pub size: Option<u64>`
  - `pub atime: SetTime`
  - `pub mtime: SetTime`
- **Args**: The input arguments for the operation.
  - `pub file: file::Handle`
  - `pub new_attr: NewAttr`
  - `pub guard: Option<Guard>`
- **Success**: The successful result wrapper.
  - `pub wcc_data: vfs::WccData`
- **Fail**: The failure result wrapper.
  - `pub error: vfs::Error`
  - `pub wcc_data: vfs::WccData`

Relations:
- **Composition**: `Args` aggregates `NewAttr` and `Option<Guard>`.
- **Composition**: `NewAttr` uses `SetTime` for time fields.
- **Association**: `Success` and `Fail` both contain `vfs::WccData`.

Global Invariants:
- **Guard Invariant**: If `Args::guard` is present, the implementation *must* perform the ctime check. A mismatch implies the operation must fail with `vfs::Error::NotSync` and attributes must be preserved (conceptually, though atomicity is not guaranteed, the intent is to reject the update).
- **Size Update Invariant**: If `NewAttr::size` is modified, the `mtime` of the file must change.

## 5. Error Model

Error Types:
- `vfs::Error`

Error Propagation Strategy:
- The method returns a `Result<Success, Fail>`. The `Fail` struct wraps the `vfs::Error` along with `vfs::WccData`.

Recoverability:
- Recoverable. The client receives the error code and the `WccData` (which contains the pre-operation attributes in `before`), allowing it to synchronize its cache and potentially retry the operation with updated `Guard` information.

Panics:
- Allowed: No
- Conditions: The trait definition does not specify panics. Implementations should return `vfs::Error` for expected failure modes.

---

## 6. Traits

List which external traits this module implements:
- **std::marker::Send**: Implemented for `SetAttr` via the `#[trait_variant::make(Send)]` macro. This allows the trait object to be sent between threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the interface for modifying file metadata within the `nfs_mamont` NFSv3 server implementation. The system contains a Virtual File System (VFS) layer that abstracts the underlying storage (e.g., local disk, network storage) from the NFS protocol logic. A typical usage scenario involves an NFS client sending a `SETATTR` request to change a file's permissions or truncate it. The server decodes this request into the `Args` struct defined in this module and invokes the `set_attr` method on the VFS implementation.

Inside the system, the following things happen and they use this module: The VFS implementation uses the `Guard` mechanism to implement optimistic locking. By checking the `ctime` (change time) provided in the guard against the actual `ctime` of the file, the server ensures that the client is acting on the latest version of the file's metadata. If the file was modified by another client in the interim, the operation fails with `vfs::Error::NotSync`, preventing data corruption. Furthermore, the `SetTime` enum allows the system to handle different client requirements for timestamp updates, such as forcing the server's time or preserving the client's time. The result of the operation, whether success or failure, always includes `vfs::WccData`. This data structure is essential for the NFS protocol's Weak Cache Consistency model, allowing the client to validate or invalidate its cached attribute data without performing a full `GETATTR` call. Without this module, the VFS would lack a standardized way to handle metadata updates, synchronization guards, and the specific WCC requirements of the NFSv3 protocol.