<!-- SPEC_HASH: 39a801d42d5063210f68ede7b1be9f67deaaebf02cab6bac6a488d3e67a51255 -->
# Module Specification

Module: mirrorfs::fs::set_attr_impl
Rust File: mirror_fs/src/fs/set_attr_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::set_attr`**: Used to implement the `SetAttr` trait for the `MirrorFS` struct. This module provides the contract (`Args`, `Success`, `Fail`) that the implementation must satisfy, including the definitions for `Guard` (optimistic locking) and `NewAttr` (attribute updates).
- **`nfs_mamont::vfs`**: Used to import the `Error` enum (specifically `NotSync` and generic I/O errors) and the `WccData` structure. These are essential for constructing the return values that conform to the NFSv3 protocol's error handling and cache consistency requirements.
- **`super::MirrorFS`**: The parent struct for which this trait implementation is defined. The implementation relies on several internal methods of `MirrorFS` (e.g., `path_for_handle`, `metadata`, `apply_set_attr`) to perform the actual file system operations. *Note: These methods are not part of the public interface of `MirrorFS` listed in the provided facts, implying they are internal implementation details.*

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Optimistic Locking via Guard Verification

**Intent:**
To prevent the "lost update" problem by ensuring that the file has not been modified by another client since the current client retrieved its attributes. This is a requirement of the NFSv3 `SETATTR` procedure.

**Inputs:**
- `args: set_attr::Args`: Contains the optional `guard` field with the expected `ctime`.

**Outputs:**
- `Result<set_attr::Success, set_attr::Fail>`: Returns `Err` with `vfs::Error::NotSync` if the check fails.

**Steps:**
1. Resolve the file handle to a concrete path using `self.path_for_handle`.
2. Retrieve the current metadata of the file using `Self::metadata`.
3. Extract the `ctime` from the current metadata.
4. If `args.guard` is `Some`, compare the current `ctime` with the guard's `ctime` using `Self::same_time`.
5. If the times do not match, return `set_attr::Fail` with `vfs::Error::NotSync`. The `wcc_data` in the failure response includes the `before` attributes (captured in step 2) and the `after` attributes (the current state).

**Edge Cases:**
- If the file handle cannot be resolved or metadata cannot be read, the operation fails immediately with empty `WccData`, and the guard check is skipped.

**Complexity:**
- Time: O(1) for the comparison, plus the cost of `path_for_handle` and `metadata` (filesystem I/O).
- Space: O(1).

**Determinism:**
- Non-deterministic (depends on filesystem state and I/O).

### Mechanism 2: Attribute Modification with WCC Tracking

**Intent:**
To apply the requested attribute changes to the file and accurately report the state of the file before and after the operation for client cache consistency (Weak Cache Consistency).

**Inputs:**
- `args: set_attr::Args`: Contains `new_attr` with the fields to modify.

**Outputs:**
- `Result<set_attr::Success, set_attr::Fail>`: Returns `Success` with `wcc_data` if the operation succeeds, or `Fail` with `wcc_data` if it fails.

**Steps:**
1. **Snapshot Before**: Capture the file's metadata immediately after resolving the handle. Convert this to `WccAttr` and store it as the `before` candidate.
2. **Apply Changes**: Invoke `Self::apply_set_attr(&path, &args.new_attr)` to perform the actual modification (e.g., truncation, mode change).
3. **Handle Failure**: If `apply_set_attr` returns an error, capture the *current* state of the file (which may be partially modified) using `Self::wcc_data`. Return `Fail` including the error and the WCC data (containing the `before` snapshot and the new `after` state).
4. **Handle Success**: If `apply_set_attr` succeeds, capture the final state of the file using `Self::wcc_data`. Return `Success` with the WCC data.

**Edge Cases:**
- **Non-atomicity**: The code explicitly handles the case where `apply_set_attr` fails but the file might have changed. It re-reads attributes for the `after` field in the `Fail` response to ensure the client sees the actual state.
- **Early Failures**: If `path_for_handle` or the initial `metadata` call fails, `WccData` is returned with `before: None` and `after: None`.

**Complexity:**
- Time: Dominated by filesystem I/O (metadata reads and writes).
- Space: O(1).

**Determinism:**
- Non-deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::set_attr`**:
 - **`SetAttr` Trait**: The module implements this trait, adhering to its contract regarding argument handling and return structures.
 - **`Guard` and `NotSync`**: The module utilizes the `Guard` struct from the trait arguments to perform the optimistic locking check and returns the `vfs::Error::NotSync` error variant as prescribed by the trait specification when the check fails.
 - **`WccData` Requirement**: The module fulfills the trait's requirement to return `WccData` in both `Success` and `Fail` scenarios, ensuring the client can update its cache even if the operation fails.

- **From `nfs_mamont::vfs`**:
 - **`Error` Enum**: The module uses `vfs::Error` variants (like `NotSync` or I/O errors propagated from internal methods) to populate the `error` field in the `Fail` struct.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on the following types from dependencies:
 - `set_attr::Args`: Input arguments.
 - `set_attr::Success`: Success return type.
 - `set_attr::Fail`: Failure return type.
 - `vfs::WccData`: Container for pre/post operation attributes.
 - `vfs::Error`: Error enumeration.

Relations:
- **Implementation**: `MirrorFS` implements `set_attr::SetAttr`.

Global Invariants:
- **WCC Consistency**: If the initial metadata lookup succeeds, the `before` field in the returned `WccData` must reflect the state of the file strictly before any modification attempts.
- **Guard Strictness**: If a `guard` is provided and the `ctime` does not match, the function must return `NotSync` and must *not* attempt to modify the file attributes.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped inside `set_attr::Fail`.
 - **`NotSync`**: Generated internally when the `ctime` check fails.
 - **IO / Other**: Propagated from internal methods like `path_for_handle`, `metadata`, or `apply_set_attr`.

Error Propagation Strategy:
- **Early Exit**: If `path_for_handle` or the initial `metadata` fails, the function returns immediately with `Fail` containing the error and empty `WccData`.
- **Late Exit**: If `apply_set_attr` fails, the function returns `Fail` containing the error and `WccData` populated with the state *after* the failed attempt (to reflect partial changes).

Recoverability:
- **`NotSync`**: The client should re-fetch attributes and retry.
- **IO Errors**: Dependent on the specific error; generally indicates a server-side issue.

Panics:
- **Allowed**: No. The implementation is designed to map all failure conditions to `Result<_, Fail>`.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::set_attr::SetAttr`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete implementation of the NFSv3 `SETATTR` procedure for the `MirrorFS` backend**. The system requires this module to bridge the abstract protocol definition (found in `nfs_mamont::vfs::set_attr`) with the specific logic of the `MirrorFS` storage driver. It is necessary because the generic `SetAttr` trait only defines *what* needs to happen (change attributes, verify guard), while this module defines *how* it happens for this specific filesystem (resolving handles, checking `ctime`, applying changes).

A typical usage scenario of the system involves an NFS client requesting to change a file's size or permissions. The RPC layer deserializes the request into `set_attr::Args` and invokes the `set_attr` method on the `MirrorFS` instance. This module then executes the sequence: it translates the opaque file handle into a local path, reads the current metadata to check if the file has been modified by someone else (Guard check), and if safe, applies the changes. Finally, it constructs a `WccData` structure containing the attributes before and after the operation, allowing the client to validate its cache without making another network call.

Inside the system, the following things happen and they use this module:
1.  **Handle Resolution**: The module calls `self.path_for_handle` to translate the NFS file handle into a filesystem path. This is critical because `MirrorFS` operates on standard paths, while the NFS protocol operates on opaque handles.
2.  **Concurrency Control**: By implementing the guard check logic (comparing `ctime`), this module ensures that `MirrorFS` adheres to the NFSv3 consistency model, preventing data corruption when multiple clients try to modify the same file simultaneously.
3.  **State Reporting**: The module meticulously captures the file state before and after the operation. This is used to populate `WccData`, which is sent back to the client. This mechanism is fundamental to the performance of NFS, as it allows clients to maintain consistent caches without excessive `GETATTR` requests.

**Uncertainty**: The implementation relies on several methods of `MirrorFS` (e.g., `path_for_handle`, `metadata`, `apply_set_attr`, `wcc_attr_from_metadata`, `same_time`, `wcc_data`) which are not listed in the public interface of `MirrorFS` in the provided `mod.rs` facts. The specification assumes these are internal helper methods available within the `MirrorFS` implementation context.