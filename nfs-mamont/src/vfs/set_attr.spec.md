<!-- SPEC_HASH: d13292a954bf9389d5ef225dde67df630a448e935c57cb5a34120006f277ddad -->
# Module Specification

Module: nfs_mamont::vfs::set_attr
Rust File: src/vfs/set_attr.rs

---

## 1. Dependencies

From the source code and context, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `WccData` struct and the `Error` enum. `WccData` is required in the return types (`Success` and `Fail`) to provide Weak Cache Consistency data to the client. `Error` is used to define the failure conditions, such as `NotSync` or `InvalidArgument`.
- **`super::file`**: Used to import the `Handle` and `Time` types. `Handle` is used in `Args` to identify the target file system object. `Time` is used in `Guard` for verification and in `SetTime` for specifying timestamp values.
- **`trait_variant::make`**: Used to transform the `SetAttr` trait into a `Send` trait object. This allows the trait to be used in asynchronous contexts where the implementor must be safe to send across threads.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `SETATTR` procedure, allowing clients to modify file attributes (metadata) such as mode, ownership, size, and timestamps.
- To implement optimistic locking via the `Guard` mechanism to prevent concurrent modifications from overwriting each other unintentionally.
- To provide flexible strategies for updating timestamps (`atime`, `mtime`), distinguishing between "don't change", "set to server time", and "set to client time".

Inputs:
- **`args: Args`**: A structure containing:
  - `file`: A `file::Handle` identifying the object to modify.
  - `new_attr`: A `NewAttr` structure specifying which attributes to change and their new values.
  - `guard`: An optional `Guard` containing the expected `ctime` of the object before modification.

Outputs:
- **`Result<Success, Fail>`**:
  - `Success`: Contains `wcc_data` (`vfs::WccData`) representing the state of the object before and after the operation.
  - `Fail`: Contains `error` (`vfs::Error`) indicating the failure reason and `wcc_data` (`vfs::WccData`) representing the state of the object (which may or may not have changed).

Steps:
1. **Guard Verification**: If `Args::guard` is `Some`, the implementation must compare the `ctime` value in the guard with the object's current `ctime`.
   - If they differ, the implementation must **not** modify any attributes and must return `Err(Fail)` with `vfs::Error::NotSync`.
2. **Attribute Application**: The implementation must apply the changes specified in `Args::new_attr`.
   - **Mode/UID/GID**: If present in `NewAttr`, update the respective fields.
   - **Size**: If present in `NewAttr`, modify the file size.
     - If `0`, truncate the file.
     - If smaller than current size, discard data beyond the new size.
     - If larger than current size, extend the file (implementation may use holes or zero bytes).
     - *Constraint*: The implementation must support extending the file size.
     - *Side Effect*: Changing the size implicitly updates the `mtime`.
   - **Timestamps**: Update `atime` and `mtime` based on the `SetTime` strategy:
     - `DontChange`: Preserve current value.
     - `ToServer`: Set to the server's current time.
     - `ToClient(time)`: Set to the specific time provided by the client.
3. **WCC Data Collection**: The implementation must capture the attributes of the object before the operation (if possible) and after the operation to populate `vfs::WccData`.
4. **Error Handling**: If an error occurs (e.g., `InvalidArgument` for UID/GID limits or 32-bit size overflow), return `Err(Fail)`. Note that the operation is **not atomic**; a failure might result in partial attribute changes.

Edge Cases:
- **Non-atomicity**: A failed `set_attr` call may leave the file in a partially modified state. The `Fail` struct includes `wcc_data` so the client can inspect the actual post-failure state.
- **Size Limits**: If the underlying implementation only supports 32-bit offsets/sizes, requesting a size larger than `u32::MAX` must result in `vfs::Error::InvalidArgument`.
- **UID/GID Limits**: If the implementation cannot store the provided `uid` or `gid`, it must return `vfs::Error::InvalidArgument`.

Complexity:
- **Time**: Dependent on the implementation (backend I/O), but the interface definition implies O(1) logic for argument validation and dispatch.
- **Space**: O(1) for the argument and result structures.

Determinism:
- **Non-deterministic**: The result depends on the state of the file system and the success of I/O operations.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
  - **`WccData`**: This module relies on the `WccData` structure to fulfill the NFSv3 requirement of returning pre- and post-operation attributes. This allows clients to validate their caches without re-reading the file.
  - **`Error`**: The module uses the `vfs::Error` enum to standardize error reporting. Specifically, it utilizes `NotSync` for guard verification failures and `InvalidArgument` for constraint violations (size/uid limits).

- **From `nfs_mamont::vfs::file`**:
  - **`Handle`**: Used to uniquely identify the target file system object for the operation.
  - **`Time`**: Used to represent timestamps for the `Guard` (optimistic locking) and for explicit client-provided times in `SetTime::ToClient`.

---

## 4. Data Model

Entities:
- **`Guard`**: A verification mechanism containing `ctime` (`file::Time`).
- **`SetTime`**: An enum defining strategies for timestamp updates.
  - Variants: `DontChange`, `ToServer`, `ToClient(file::Time)`.
- **`NewAttr`**: A container for attribute updates.
  - Fields: `mode` (Option<u32>), `uid` (Option<u32>), `gid` (Option<u32>), `size` (Option<u64>), `atime` (SetTime), `mtime` (SetTime).
- **`Args`**: The input arguments for the `set_attr` operation.
  - Fields: `file` (file::Handle), `new_attr` (NewAttr), `guard` (Option<Guard>).
- **`Success`**: The successful result wrapper.
  - Fields: `wcc_data` (vfs::WccData).
- **`Fail`**: The failure result wrapper.
  - Fields: `error` (vfs::Error), `wcc_data` (vfs::WccData).

Relations:
- **Composition**: `Args` aggregates `NewAttr` and `Guard`.
- **Association**: `Success` and `Fail` both contain `vfs::WccData`.

Global Invariants:
- **Guard Semantics**: If `Args::guard` is present and the `ctime` check fails, the file attributes must remain unchanged.
- **Size Modification**: Modifying the file size via `NewAttr::size` must result in an update to the file's `mtime`.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type returned within the `Fail` struct.

Error Propagation Strategy:
- **Direct Return**: Errors are returned wrapped in the `Fail` struct. The `Fail` struct also includes `wcc_data` to reflect the state of the object after the failed attempt (which may be partially modified).

Recoverability:
- **`vfs::Error::NotSync`**: Indicates a race condition (the file was modified by another client). The client should typically re-fetch the current attributes and retry the operation.
- **`vfs::Error::InvalidArgument`**: Indicates a client error (e.g., requested size too large, invalid UID). The client should correct the request.
- **Other Errors**: Standard I/O or permission errors where recovery depends on the specific error code.

Panics:
- **Allowed**: No. The interface defines a contract for returning errors via `Result`, not for panicking.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`SetAttr`**: An asynchronous trait defining the `set_attr` method. It is marked `Send` via `trait_variant`, allowing it to be used as a trait object in multi-threaded async contexts.

---

## 7. Overview

This module is used in order to **define the contract for modifying file system object attributes** within the `nfs_mamont` NFSv3 server. The system requires a specialized interface for this operation because the NFSv3 `SETATTR` procedure has unique semantics that differ from a simple "write metadata" command. Specifically, it supports partial updates (only changing the fields provided by the client), optimistic locking (checking `ctime` to prevent lost updates), and complex timestamp handling (setting time to server time vs. client time).

A typical usage scenario of the system involves a client wishing to truncate a file or change its permissions. The RPC layer receives the request, deserializes the arguments into the `Args` struct (which includes the `NewAttr` and potentially a `Guard`), and invokes the `set_attr` method on the VFS backend. The backend implementation checks the guard (if present) to ensure the file hasn't changed since the client last looked at it. If valid, it applies the updates—handling the specific logic for truncating or extending the file—and returns the `Success` struct containing `WccData`. This `WccData` is crucial because it allows the client to update its cache efficiently without performing a separate `GETATTR` call.

Inside the system, the following things happen and they use this module:
1.  **Optimistic Concurrency Control**: The `Guard` struct provides the mechanism for the "Verify" step in NFSv3 `SETATTR`. If the `ctime` in the guard does not match the file's current `ctime`, the backend returns `vfs::Error::NotSync`. This prevents the "lost update" problem where two clients overwrite each other's changes.
2.  **Protocol Compliance**: The `SetTime` enum maps directly to the NFSv3 distinctions for how to handle `atime` and `mtime` (e.g., `SET_TO_SERVER_TIME` vs `SET_TO_CLIENT_TIME`). This module ensures that the backend implementation explicitly handles these cases rather than guessing the client's intent.
3.  **Partial State Reporting**: By requiring `WccData` in both `Success` and `Fail` results, this module enforces the NFSv3 requirement that the client must be informed about the state of the object even if the operation failed (due to the non-atomic nature of the operation).

Without this module, the VFS layer would lack a standardized way to handle the nuanced requirements of attribute modification in NFSv3, leading to potential inconsistencies in cache handling, race conditions, and incorrect timestamp management.