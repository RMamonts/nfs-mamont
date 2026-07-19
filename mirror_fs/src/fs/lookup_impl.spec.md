<!-- SPEC_HASH: afefc29e30761ad265f6ba1fb085b23646f83e6df4af0f3df3a6f250974768d7 -->
# Module Specification

Module: mirrorfs::fs::lookup_impl
Rust File: mirror_fs/src/fs/lookup_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::lookup`**: Used to import the `Lookup` trait, `Args`, `Success`, and `Fail` types. This module provides the concrete implementation of the `Lookup` trait for the `MirrorFS` struct, enabling the filesystem to respond to NFSv3 LOOKUP requests.
- **`std::path::PathBuf`**: Used to construct and manipulate file system paths. The implementation resolves abstract NFS handles into concrete `PathBuf` objects to perform directory traversal (handling `.` and `..`) and metadata checks.
- **`super::MirrorFS`**: The struct for which the `Lookup` trait is implemented. The implementation relies on internal methods of `MirrorFS` (e.g., `path_for_handle`, `handle_for_path`, `exported_root_path`) to translate between the VFS abstraction and the underlying storage.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Handle-to-Path Resolution and Validation

**Intent:**
To translate the abstract NFS file handle (provided in `args.parent`) into a concrete file system path, verify that it points to a valid directory, and ensure the caller has permission to access it. This mechanism bridges the gap between the stateless NFS protocol (which uses handles) and the stateful OS filesystem (which uses paths).

**Inputs:**
- `args: lookup::Args`: Contains the `parent` handle and the `name` to look up.

**Outputs:**
- `Result<lookup::Success, lookup::Fail>`: Returns the handle and attributes of the requested object on success, or an error and directory attributes on failure.

**Steps:**
1. **Resolve Parent Handle**: Calls `self.path_for_handle(&args.parent).await` to convert the parent handle into a `PathBuf`. If this fails, returns `Fail` with `dir_attr: None`.
2. **Fetch Parent Metadata**: Calls `Self::metadata(&parent_path)` to retrieve the directory's metadata. If this fails, returns `Fail` with `dir_attr: None`.
3. **Validate Directory**: Converts metadata to attributes (`Self::attr_from_metadata`) and checks if the parent is a directory using `Self::validate_directory`. If validation fails, returns `Fail` with `dir_attr: Some(parent_attr)` to support Weak Cache Consistency (WCC).
4. **Construct Child Path**:
   - If `name` is `.`, the child path is the parent path.
   - If `name` is `..`, the implementation checks if the parent is the `exported_root_path`. If it is, the child path remains the parent path (preventing escape from the export). Otherwise, it uses `parent_path.parent()`.
   - For any other name, it appends the name to the parent path.
5. **Resolve Child**: Fetches metadata for the child path and converts it back to a handle using `self.handle_for_path(&child_path).await`.
6. **Return Success**: Returns `Success` containing the child handle, child attributes, and parent attributes.

**Edge Cases:**
- **Export Root Escape**: When looking up `..` at the root of the exported filesystem, the implementation returns the root itself rather than the parent directory, ensuring the client cannot traverse above the shared export.
- **Attribute Availability**: Directory attributes (`dir_attr`) are included in the `Fail` response only if the parent metadata was successfully retrieved. This allows the client to update its cache even if the specific lookup operation failed.

**Complexity:**
- Time: O(1) for path manipulation, plus the cost of underlying filesystem syscalls (metadata lookup, handle generation).
- Space: O(1) (allocates a few `PathBuf` instances).

**Determinism:**
- Deterministic, assuming the underlying filesystem state does not change during the execution of the function.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::lookup`**:
 - **`Lookup` Trait**: Defines the asynchronous interface (`async fn lookup`) that this module implements. It enforces the separation between the NFS protocol logic (arguments/results) and the storage backend logic.
 - **`Args`, `Success`, `Fail`**: The data structures used to transport data into and out of the implementation. `Success` and `Fail` specifically carry the Weak Cache Consistency (WCC) data (`dir_attr`), which this module populates based on the state of the parent directory.

- **From `mirrorfs::fs`**:
 - **`MirrorFS`**: The context for the implementation. While the public facts list `new`, `root_handle`, and `handle_for_path`, the code relies on additional mechanisms:
   - `path_for_handle`: Assumed to exist (async) to map NFS handles to `PathBuf`.
   - `exported_root_path`: Assumed to exist (async) to determine the boundary of the shared filesystem for `..` handling.
   - Internal helpers (`Self::metadata`, `Self::attr_from_metadata`, `Self::validate_directory`): Assumed to exist to abstract standard filesystem operations.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on entities defined in `nfs_mamont::vfs::lookup` and `std::path`.

Relations:
- **Implementation Relation**: `MirrorFS` implements `lookup::Lookup`.
- **Mapping Relation**: `file::Handle` $\leftrightarrow$ `PathBuf`. The module orchestrates the conversion between these two representations using methods on `MirrorFS`.

Global Invariants:
- **Export Containment**: The lookup operation must never return a handle for a path outside the exported root. This is enforced by the specific logic handling the `..` name component.
- **Directory Validity**: The operation guarantees that if `Success` is returned, the `parent` handle referred to a valid directory at the time of the lookup.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped inside `lookup::Fail`. The specific error variants depend on the failures encountered in `MirrorFS` methods (e.g., `NoEntry` if a path doesn't exist, `IO` for disk errors, `NotDir` if the parent is not a directory).

Error Propagation Strategy:
- **Early Return**: The function uses a series of `match` statements. If any step fails (resolving the handle, reading metadata, validating the directory), it immediately returns a `lookup::Fail` struct.
- **Attribute Preservation**: When returning `Fail`, the module attempts to include `dir_attr` (parent attributes) if the parent metadata was successfully retrieved earlier in the process. This supports the NFSv3 Weak Cache Consistency model.

Recoverability:
- **Dependent on Error**: Recoverability is determined by the `vfs::Error` variant returned. For example, `JUKEBOX` implies the client should retry, while `StaleFile` implies the client needs to remount or re-lookup the path.

Panics:
- **Allowed**: No explicit panics are present in the code. All error paths return `Result`.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::lookup::Lookup`**: Implemented for `mirrorfs::fs::MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete implementation of the NFSv3 LOOKUP procedure for the `MirrorFS` backend**. The system contains a complex architecture where the `nfs_mamont` server defines generic interfaces (traits) for file system operations, but relies on specific implementations (like `MirrorFS`) to perform the actual work on storage. This module is necessary because it defines exactly how `MirrorFS` translates the abstract concept of "looking up a name in a directory" (defined by handles and names in the NFS protocol) into concrete file system operations (paths, metadata checks, and handle generation).

A typical usage scenario of the system involves an NFS client requesting to access a file by its name within a directory. The `nfs_mamont` RPC layer receives the request, extracts the directory handle and filename, and calls the `lookup` method on the `MirrorFS` instance (via the `Vfs` trait). This module executes the logic: it finds the path corresponding to the directory handle, checks if it is valid, constructs the path for the child (handling special cases like `.` and `..`), and generates a new handle for the child.

Inside the system, the following things happen and they use this module:
1.  **Path Translation**: The module converts the stateless NFS handles into stateful OS paths. This is critical because `MirrorFS` (presumably) wraps a standard OS filesystem, which requires paths to perform operations like `stat` or `open`.
2.  **Security Enforcement**: The module explicitly handles the `..` (parent directory) case to ensure that clients cannot traverse above the `exported_root_path`. This enforces the boundary of the NFS export, preventing unauthorized access to the rest of the server's filesystem.
3.  **Cache Consistency**: The module carefully manages the population of the `dir_attr` field in both `Success` and `Fail` results. By fetching the parent directory's metadata early and reusing it, it ensures that the client receives up-to-date directory attributes, allowing the client to validate its cache of directory contents even if the specific file lookup failed.

**Uncertainty**: The provided public interfaces for `MirrorFS` do not list `path_for_handle`, `exported_root_path`, or the helper methods `metadata`, `attr_from_metadata`, and `validate_directory`. The analysis assumes these methods exist as `async` or inherent methods on `MirrorFS` or within the `super` scope, as they are invoked in the source code.