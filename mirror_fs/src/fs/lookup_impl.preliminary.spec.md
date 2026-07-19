<!-- SPEC_HASH: afefc29e30761ad265f6ba1fb085b23646f83e6df4af0f3df3a6f250974768d7 -->
# Module Specification

Module: mirrorfs::fs::lookup_impl
Rust File: mirror_fs/src/fs/lookup_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::path::PathBuf`**: Used to construct and manipulate file system paths. Specifically, it is used to resolve the target path by joining the parent directory path with the child name, and to handle parent directory traversal (`..`).
- **`nfs_mamont::vfs::lookup`**: Used to import the `Lookup` trait and its associated types (`Args`, `Success`, `Fail`). This module provides the interface that this implementation satisfies, defining the contract for resolving a file name within a directory to a file handle.
- **`super::MirrorFS`**: Used as the implementation target. The `lookup` method is implemented for this struct, relying on its internal state and helper methods to map between opaque file handles and concrete file system paths.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Path Resolution and Handle Translation

**Intent:**
To implement the NFS `LOOKUP` procedure for the `MirrorFS` backend. This involves translating an abstract directory handle and a filename into a concrete filesystem path, validating the path, and returning a new handle for the target object. The implementation must handle special directory entries (`.` and `..`) and ensure that the operation respects the boundaries of the exported file system root.

**Inputs:**
- `args: lookup::Args`: Contains the `parent` handle and the `name` to look up.

**Outputs:**
- `Result<lookup::Success, lookup::Fail>`: Contains the target file handle, its attributes, and the parent directory's attributes on success, or an error and optional parent attributes on failure.

**Steps:**
1. **Parent Resolution**: The method calls `self.path_for_handle(&args.parent)` to convert the parent handle into a `PathBuf`. If this fails, it returns `Fail` with `dir_attr: None`.
2. **Parent Validation**: It retrieves metadata for the parent path using `Self::metadata`. If successful, it converts this to `Attr` and validates that the parent is a directory using `Self::validate_directory`. If validation fails, it returns `Fail` with `dir_attr: Some(parent_attr)`.
3. **Path Construction**:
   - If `name` is `.`, the child path is set to the parent path.
   - If `name` is `..`, the method retrieves the `exported_root_path`. If the parent path is the root, the child path remains the root. Otherwise, it resolves to the parent's parent directory.
   - For any other name, the child path is constructed by appending the name to the parent path.
4. **Child Resolution**: It retrieves metadata for the resolved child path. If this fails (e.g., file not found), it returns `Fail` with `dir_attr: Some(parent_attr)`.
5. **Handle Generation**: It converts the child path back to a file handle using `self.handle_for_path`. If this fails, it returns `Fail` with `dir_attr: Some(parent_attr)`.
6. **Success**: It returns `Success` containing the child handle, child attributes, and parent attributes.

**Edge Cases:**
- **Root Traversal**: When looking up `..` at the exported root, the implementation ensures the path does not escape the export by returning the root path itself.
- **Attribute Availability**: The `dir_attr` field in the `Fail` struct is populated with `Some(parent_attr)` only if the parent metadata was successfully retrieved and validated. If the parent handle itself is invalid or metadata retrieval fails, `dir_attr` is `None`.

**Complexity:**
- Time: O(1) to O(N) depending on the underlying filesystem operations (metadata access, path resolution).
- Space: O(1) (allocates `PathBuf` instances).

**Determinism:**
- Deterministic (given the state of the filesystem).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::lookup`**:
 - **`Args`, `Success`, `Fail`**: These structures define the input and output contract. The implementation uses `Args` to receive the target handle and name, and constructs `Success` or `Fail` to return the result, ensuring compliance with the NFSv3 protocol.
 - **`Error`**: The `vfs::Error` enum is used within the `Fail` struct to signal specific failure conditions (e.g., `NoEntry`, `NotDir`, `IO`) to the upper layers.

- **From `mirrorfs::fs` (Assumed based on usage)**:
 - **`path_for_handle`**: A method (likely internal) used to resolve the abstract `Handle` into a concrete `PathBuf`. This is critical for bridging the NFS protocol's opaque handles with the OS's path-based filesystem.
 - **`handle_for_path`**: A method used to generate a new `Handle` for a given `PathBuf`. This allows the implementation to return a valid identifier to the client for the looked-up object.
 - **`metadata`**: A helper function (likely `std::fs::metadata` or a wrapper) used to retrieve file system metadata (inode, type, permissions) required to construct `Attr` objects.
 - **`attr_from_metadata`**: A helper function used to convert raw OS metadata into the `Attr` structure defined by the VFS layer.
 - **`validate_directory`**: A helper function used to ensure the parent handle actually refers to a directory, enforcing protocol semantics.
 - **`exported_root_path`**: A method providing the root path of the export, used to constrain `..` traversal.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It implements the `Lookup` trait for the existing `MirrorFS` struct.

Relations:
- **Implementation**: `MirrorFS` implements `nfs_mamont::vfs::lookup::Lookup`.

Global Invariants:
- **Export Root Constraint**: The implementation ensures that a lookup of `..` from the exported root returns the root handle, preventing the client from traversing above the shared directory.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type returned within `lookup::Fail`. This includes standard NFS errors like `NoEntry` (if the child does not exist), `NotDir` (if the parent is not a directory), `IO` (if filesystem access fails), or `StaleFile` (if the parent handle is invalid).

Error Propagation Strategy:
- **Early Return with Context**: The function uses a series of `match` statements to check results of internal operations. If an error occurs, it immediately returns `Err(lookup::Fail)`.
- **Attribute Preservation**: The implementation attempts to preserve the parent directory's attributes (`dir_attr`) in the error response whenever possible (i.e., if parent metadata was successfully retrieved). This supports the Weak Cache Consistency (WCC) model of NFSv3, allowing the client to update its cache even if the specific lookup operation failed.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` returned. For example, `IO` errors might be transient, while `NoEntry` implies the file does not exist and retrying immediately is futile.

Panics:
- **Allowed**: No. The implementation uses `Result` types for all fallible operations and does not explicitly unwrap or expect values that could panic.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::lookup::Lookup`**: Implemented for `MirrorFS`. This trait defines the asynchronous `lookup` method used to resolve file names to handles.

---

## 7. Overview

This module is used in order to **bridge the generic NFS VFS interface with the specific path-based logic of the `MirrorFS` backend**. The system contains a complex architecture where the storage backend (`MirrorFS`) must conform to the abstract `Vfs` trait defined in `nfs_mamont`. This module is necessary because it provides the concrete logic for the `LOOKUP` operation, translating the abstract request (handle + name) into a concrete filesystem operation (path resolution + metadata check).

A typical usage scenario of the system involves an NFS client requesting to access a file within a directory. The client sends a `LOOKUP` request with a directory handle and a filename. The `nfs_mamont` RPC layer receives this request and calls the `lookup` method on the `MirrorFS` instance (which acts as the `Vfs`). This module's implementation executes, resolving the handle to a path, checking if the target exists, and returning a new handle. This allows the client to "walk" the directory tree without knowing the server's internal file paths.

Inside the system, the following things happen and they use this module:
1.  **Handle Translation**: The module relies on `MirrorFS`'s internal mapping between opaque handles and `PathBuf`s. This decouples the client's view of the filesystem (handles) from the server's implementation (paths).
2.  **Security and Boundary Enforcement**: The module explicitly handles the `..` case to ensure that the client cannot traverse past the exported root directory. This is a critical security feature for multi-tenant or shared file systems.
3.  **Protocol Compliance**: By returning `dir_attr` in both success and failure cases (where available), the module implements the NFSv3 Weak Cache Consistency model. This allows the client to validate its cache of the directory contents, improving performance and reducing network traffic.

The critical aspect of this module is the **handling of the `..` edge case**. Unlike a standard filesystem lookup which might simply traverse to the parent directory, this implementation checks if the current directory is the `exported_root_path`. If it is, it returns the root itself, effectively "jailing" the client within the export. This ensures that the logical view of the filesystem presented to the client matches the exported mount point, regardless of the physical directory structure on the server.