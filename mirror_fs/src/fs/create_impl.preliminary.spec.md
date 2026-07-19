<!-- SPEC_HASH: 5607db70b913c79731aac5a74ea708458c397273d0288314b8543ad26bce4443 -->
# Module Specification

Module: mirrorfs::fs::create_impl
Rust File: src/fs/create_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs::OpenOptions`**: Used to asynchronously open and create files on the underlying local file system. It provides the configuration for write access, creation flags, and exclusive creation modes required by the NFS semantics.
- **`nfs_mamont::vfs`**: Used to import the `Error` enum and `WccData` struct. `Error` is used to wrap I/O errors or protocol-specific failures (like `Exist`), while `WccData` is used to report the state of the parent directory before and after the operation for cache consistency.
- **`nfs_mamont::vfs::create`**: Used to import the `Create` trait and its associated types (`Args`, `Success`, `Fail`, `How`). This module provides the concrete implementation of the `create` method for the `MirrorFS` struct, adhering to the contract defined here.
- **`super::{MirrorFS, DEFAULT_SET_ATTR}`**: Used to access the `MirrorFS` struct for which this implementation is defined, and a constant `DEFAULT_SET_ATTR` which provides default attributes to be applied in specific creation scenarios (e.g., `Exclusive` mode).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `vfs::create::Create` for `MirrorFS`

**Intent:**
To execute the logic required to create a regular file on the local filesystem backing `MirrorFS`, while strictly adhering to the NFSv3 creation semantics (Unchecked, Guarded, Exclusive) and maintaining Weak Cache Consistency (WCC) for the parent directory.

**Inputs:**
- `&self`: A reference to the `MirrorFS` instance.
- `args: create::Args`: Arguments containing the target directory (`object.dir`), the file name (`object.name`), and the creation mode (`how`).

**Outputs:**
- `Result<create::Success, create::Fail>`:
 - `Success`: Contains the new file handle, file attributes, and directory WCC data.
 - `Fail`: Contains a `vfs::Error` and directory WCC data.

**Steps:**
1. **Name Validation**: Calls `Self::ensure_name_allowed` with the file name. If validation fails, returns `Fail` immediately with empty WCC data.
2. **Path Resolution**: Calls `self.path_for_handle` to convert the directory handle into a filesystem path. If this fails, returns `Fail` with empty WCC data.
3. **Pre-Operation State Capture**: Retrieves metadata for the directory path using `Self::metadata`. If this fails, returns `Fail` with empty WCC data. Stores the "before" attributes for WCC.
4. **Directory Validation**: Validates the directory path using `Self::validate_directory`. If it fails, returns `Fail` with WCC data containing the "before" attributes.
5. **Existence Check**: Constructs the full child path and checks if the entry exists using `std::fs::symlink_metadata` (synchronous check).
6. **Creation Logic (Branching on `args.how`)**:
 - **Unchecked**: If the file does not exist, it opens the file with `create(true)` and `truncate(false)`. If it exists, it skips opening. It uses the attributes provided in `args.how`.
 - **Guarded**: If the file exists, it returns `Fail` with `vfs::Error::Exist`. If it does not exist, it opens the file with `create_new(true)`. It uses the attributes provided in `args.how`.
 - **Exclusive**: Attempts to open the file with `create_new(true)`.
 - If successful, it stores the exclusive verifier via `Self::store_exclusive_verifier`.
 - If the error is `AlreadyExists`, it checks the verifier using `Self::check_exclusive_verifier`. If the verifier does not match, it returns `Fail` with `vfs::Error::Exist`. If it matches, it proceeds (idempotent success).
 - It uses `DEFAULT_SET_ATTR` for this mode.
7. **Attribute Application**: Calls `Self::apply_set_attr` to apply the determined attributes to the file. If this fails, returns `Fail` with WCC data.
8. **Post-Operation State Capture**: Retrieves metadata for the newly created/modified file using `Self::metadata` and converts it to `Attr`.
9. **Handle Generation**: Generates a new file handle for the child path using `self.handle_for_path`.
10. **Response Construction**: Returns `Success` containing the file handle, attributes, and WCC data (calculated from the directory's "before" state and current state).

**Edge Cases:**
- **Idempotency in Exclusive Mode**: If a file exists in `Exclusive` mode, the operation does not fail immediately; it checks the verifier. If the verifier matches the stored data, the operation succeeds (allowing client retries), otherwise it fails with `Exist`.
- **Unchecked with Existing File**: In `Unchecked` mode, if the file already exists, the code does not attempt to create or truncate it; it simply proceeds to apply attributes. This aligns with the "create or ignore" semantics often expected, though strictly NFSv3 UNCHECKED implies "create without checking for duplicate", effectively succeeding if it exists or creating it if it doesn't.

**Complexity:**
- Time: O(1) relative to data structures, but dominated by filesystem I/O latency (metadata lookups, file creation).
- Space: O(1) (excluding path buffers and internal FS structures).

**Determinism:**
- Non-deterministic. Depends on the current state of the filesystem (existence of files, I/O errors).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::create`**:
 - **`How` Enum**: The logic of this module is fundamentally driven by the variants of `How` (`Unchecked`, `Guarded`, `Exclusive`). The implementation must branch based on this enum to satisfy the specific NFS protocol requirements for each mode.
 - **`Success` and `Fail` Structs**: The module constructs these specific return types. `Success` requires populating `file` (Handle), `attr` (Attr), and `wcc_data`, while `Fail` requires populating `error` and `wcc_data`.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: The module is responsible for capturing the state of the parent directory before the operation and calculating the final state after the operation to populate this structure. This is critical for the client's cache coherency.
 - **`Error`**: The module maps internal I/O errors (from `tokio::fs` or `std::fs`) and logical conditions (like name validation failure) into `vfs::Error` variants (e.g., `IO`, `Exist`).

---

## 4. Data Model

Entities:
- This module defines no public entities. It implements the `create` method for the `MirrorFS` struct, which is defined in the parent module.

Relations:
- **Implementation**: `MirrorFS` implements `vfs::create::Create`.

Global Invariants:
- **WCC Consistency**: If the directory metadata is successfully retrieved before the operation, the `before` field in the returned `WccData` must be `Some`. If the operation fails before directory metadata is retrieved, `before` is `None`.
- **Verifier Storage**: In `Exclusive` mode, the implementation assumes `Self::store_exclusive_verifier` persists the verifier in a way that `Self::check_exclusive_verifier` can later retrieve and validate it.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type returned within `create::Fail`. Specific variants observed include `Exist` (for duplicate files in Guarded/Exclusive modes) and errors derived from I/O operations via `Self::io_error_to_vfs`.

Error Propagation Strategy:
- **Early Return**: The function uses a series of `match` and `if let` blocks to check intermediate results. If any step fails (name validation, path resolution, metadata retrieval, file opening, attribute setting), it immediately returns a `create::Fail` struct containing the error and the available WCC data.

Recoverability:
- **`vfs::Error::Exist`**: Indicates the file already exists. In `Guarded` mode, this is a hard failure for the specific request. In `Exclusive` mode, it might be a failure or a success depending on the verifier check.
- **I/O Errors**: Generally indicate transient or persistent filesystem issues. The client may retry depending on the specific error code.

Panics:
- **Allowed**: No. The implementation is designed to return `Result` types for all foreseeable error conditions.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::create::Create`**: The module provides an `async fn create` implementation for `MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete filesystem logic for the NFSv3 `CREATE` procedure within the `MirrorFS` backend**. The system requires this implementation because the `nfs_mamont` library defines the abstract protocol interface (the `Create` trait), but it does not know how to interact with the specific storage medium used by `MirrorFS` (which appears to be a local filesystem mirror). This module bridges that gap by translating high-level NFS requests into low-level file operations.

The system contains a complex separation of concerns where the protocol logic (handling `How` modes, verifiers, and WCC data) is defined in the `nfs_mamont` crate, and the storage logic is implemented in the `mirrorfs` crate. This module is necessary to ensure that `MirrorFS` can correctly handle file creation requests from NFS clients, respecting the specific semantics of `Unchecked`, `Guarded`, and `Exclusive` creation modes which are more nuanced than standard POSIX file creation.

A typical usage scenario of the system involves an NFS client requesting to create a file. The request arrives at the server, is dispatched to the `MirrorFS` backend, and this `create` method is invoked. The method resolves the directory handle to a local path, checks if the file already exists, and then performs the creation based on the requested mode. For example, in `Exclusive` mode, it ensures that the file is created atomically or that the request is a retry of a previously successful creation (verified by the verifier). Finally, it returns the new file handle and attributes to the client, along with WCC data to keep the client's directory cache synchronized.

Inside the system, the following things happen and they use this module:
1.  **Protocol Enforcement**: The module explicitly handles the branching logic for `How::Unchecked`, `How::Guarded`, and `How::Exclusive`. This ensures that the backend does not violate NFS protocol guarantees, such as the requirement to fail with `Exist` in `Guarded` mode if the file is present.
2.  **Cache Consistency Management**: The module captures the directory's metadata *before* attempting to create the file. This is crucial for constructing the `WccData` (Weak Cache Consistency data) returned to the client, allowing the client to validate its cached directory contents without re-reading the entire directory.
3.  **Idempotency Handling**: In `Exclusive` mode, the module implements logic to check a stored verifier. This allows the NFS server to handle client retries safely; if the file exists and the verifier matches, the operation is treated as a success rather than a failure, which is a key requirement for robust NFS operation over unreliable networks.

**Assumptions**:
- The methods `path_for_handle`, `handle_for_path`, `ensure_name_allowed`, `metadata`, `attr_from_metadata`, `wcc_attr_from_metadata`, `validate_directory`, `io_error_to_vfs`, `wcc_data`, `store_exclusive_verifier`, `check_exclusive_verifier`, and `apply_set_attr` are assumed to exist on `MirrorFS` or in its scope (`super`), as they are called but not defined in the provided snippet. Their behavior is inferred from their names and usage context.