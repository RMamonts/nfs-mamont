<!-- SPEC_HASH: 7fb3c54f8d601ff52e92286d5ebf79b07b1b80a066393492c882a0cb99f35d6c -->
# Module Specification

Module: mirrorfs::fs::symlink_impl
Rust File: mirror_fs/src/fs/symlink_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::symlink`**: Used to import the `Symlink` trait and its associated types (`Args`, `Success`, `Fail`). This module provides the concrete implementation of this trait for the `MirrorFS` struct, bridging the high-level NFS protocol definition with the low-level file system operations.
- **`nfs_mamont::vfs`**: Used to import the `WccData` struct and the `Error` enum. These are required to construct the return values for the `symlink` operation, specifically for reporting Weak Cache Consistency data and mapping OS errors to NFS status codes.
- **`super::MirrorFS`**: The struct for which the `Symlink` trait is being implemented. The implementation relies on several helper methods assumed to exist on `MirrorFS` (e.g., `path_for_handle`, `handle_for_path`, `ensure_name_allowed`, `io_error_to_vfs`) to handle path resolution, validation, and error translation.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Implementation of `symlink::Symlink` for `MirrorFS`

**Intent:**
To execute the creation of a symbolic link on the local file system in response to an NFSv3 `SYMLINK` request. This involves validating the request, resolving the abstract directory handle to a concrete path, performing the OS-level system call, and gathering the necessary metadata and handles to satisfy the NFS response contract.

**Inputs:**
- `args: symlink::Args`: Contains the target directory handle (`args.object.dir`), the new link name (`args.object.name`), and the target path content (`args.path`).

**Outputs:**
- `Result<symlink::Success, symlink::Fail>`:
 - `Success`: Contains the new file handle, attributes of the new link, and `WccData` for the parent directory.
 - `Fail`: Contains the error that occurred and the `WccData` for the parent directory (if available).

**Steps:**
1. **Name Validation**: Calls `Self::ensure_name_allowed` with the link name. If validation fails, returns `symlink::Fail` with the error and empty `WccData`.
2. **Path Resolution**: Calls `self.path_for_handle` to convert the abstract directory handle into a concrete filesystem path. If this fails, returns `symlink::Fail` with the error and empty `WccData`.
3. **Pre-Operation Snapshot**: Captures the metadata of the parent directory using `std::fs::symlink_metadata` to establish the "before" state for Weak Cache Consistency (`WccData`).
4. **Link Construction**: Constructs the full path for the new link by joining the directory path and the link name.
5. **System Call**: Invokes `std::os::unix::fs::symlink` to create the symbolic link pointing to `args.path`.
 - If this fails, maps the `std::io::Error` to a `vfs::Error` using `Self::io_error_to_vfs`, captures the "after" state of the directory, and returns `symlink::Fail`.
6. **Post-Operation Metadata**: Retrieves the metadata of the newly created link using `Self::metadata`. If this fails, returns `symlink::Fail` with the directory's `WccData`.
7. **Handle Generation**: Generates a new file handle for the created link using `self.handle_for_path`. If this fails, returns `symlink::Fail`.
8. **Success Response**: Returns `symlink::Success` containing the new handle, attributes, and the calculated `WccData` for the parent directory.

**Edge Cases:**
- **Early Failures**: If name validation or path resolution fails, the `WccData` in the error response is empty (`before: None, after: None`), as the directory was not accessed or its state could not be determined.
- **Atomicity**: The implementation relies on the atomicity guarantees of the underlying OS `symlink` call. If the OS call succeeds, the link exists; if it fails, it does not.

**Complexity:**
- Time: O(1) relative to data structures, dominated by system call latency (filesystem I/O).
- Space: O(1) (allocations for paths and metadata structures).

**Determinism:**
- Non-deterministic. Depends on the state of the underlying filesystem (e.g., permissions, disk space, existence of parent).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::symlink`**:
 - **`Symlink` Trait**: Defines the contract that this module fulfills. It mandates the specific signature `async fn symlink` and the structure of `Args`, `Success`, and `Fail`.
 - **`Args`**: Provides the input data structure, specifically `object` (directory and name) and `path` (link content).
 - **`Success` and `Fail`**: Define the required return payloads, ensuring that `WccData` is always returned to the client for cache coherency.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: Used to construct the `dir_wcc` field in `Fail` and `wcc_data` field in `Success`. This module populates this structure by comparing directory metadata before and after the operation.
 - **`Error`**: Used as the error type within `Fail`. The module converts `std::io::Error` into `vfs::Error` (via an assumed helper) to ensure the error is protocol-compliant.

- **From `mirrorfs::fs` (Assumed based on usage)**:
 - **`path_for_handle`**: Assumed mechanism to translate the abstract `Handle` into a concrete `PathBuf` for the directory.
 - **`handle_for_path`**: Assumed mechanism to generate a new `Handle` for the newly created symbolic link.
 - **`ensure_name_allowed`**: Assumed validation logic to check if the link name is permissible (e.g., rejecting "." or "..").

---

## 4. Data Model

Entities:
- This module defines no public structs or enums. It operates entirely on types defined in `nfs_mamont::vfs` and `nfs_mamont::vfs::symlink`.

Relations:
- **Implementation**: `MirrorFS` implements `symlink::Symlink`.

Global Invariants:
- **WCC Consistency**: If the directory path is successfully resolved, the `WccData` returned in both `Success` and `Fail` cases must reflect the state of the directory *before* the operation (if retrievable) and *after* the operation (if the operation was attempted).

## 5. Error Model

Error Types:
- **`symlink::Fail`**: The wrapper error type returned to the caller. It contains a `vfs::Error` and `WccData`.

Error Propagation Strategy:
- **Early Return**: The function checks for errors at every step (validation, resolution, creation, metadata retrieval). If any step fails, it immediately returns a `symlink::Fail` wrapping the specific error.
- **Error Mapping**: OS-level errors (`std::io::Error`) resulting from the `symlink` system call are mapped to `vfs::Error` using an assumed `Self::io_error_to_vfs` method.

Recoverability:
- **Dependent on `vfs::Error`**:
 - `Exist`: Returned if the name is invalid (e.g., "." or "..") or if the file already exists (implied by OS error mapping). The client must change the name.
 - `IO`: Indicates a disk or permission error. The client may retry if the condition is transient.
 - `StaleFile`: Returned if the directory handle is invalid. The client must perform a new lookup.

Panics:
- **Allowed**: No explicit panics in the provided code. However, unwrapping or expecting on internal logic (not shown) could cause panics.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::symlink::Symlink`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **provide the concrete filesystem logic for creating symbolic links** within the `MirrorFS` backend. The `nfs_mamont` library defines the abstract protocol for NFSv3 operations, including the `SYMLINK` procedure. This module is necessary because the abstract protocol does not know how to interact with the specific storage medium (in this case, the local Unix filesystem). It acts as the adapter that translates the high-level request (create a link with this name in this directory pointing to that target) into a sequence of low-level operations (resolve paths, call `symlink()`, read metadata).

The system contains a layered architecture where the top layer handles the network protocol (RPC), the middle layer defines the VFS interface (`nfs_mamont::vfs`), and the bottom layer implements the storage backend (`MirrorFS`). This module resides in the bottom layer. It is critical for ensuring that the server can fulfill client requests to create symbolic links, which is a fundamental feature of POSIX-like filesystems exposed over NFS.

A typical usage scenario of the system involves a client sending an NFS `SYMLINK` request. The request travels through the RPC decoder, which calls the `symlink` method on the `MirrorFS` instance. This module then executes the logic defined in the "Mechanisms" section: it validates the name, finds the real path on disk, creates the link, and gathers the attributes to send back to the client.

Inside the system, the following things happen and they use this module:
1. **Path Translation**: The module uses `path_for_handle` to convert the opaque NFS file handle (which might be an inode number or a hash) into a real filesystem path. This decouples the NFS protocol from the server's physical directory layout.
2. **Cache Coherency**: By capturing directory metadata before and after the operation, this module enables the Weak Cache Consistency mechanism defined in the NFS specification. This allows clients to validate their cached directory contents without re-reading the entire directory.
3. **Error Normalization**: Filesystem errors (like "Permission denied" or "No space left on device") are caught and translated into standard NFS error codes. This ensures that the client receives a compliant error response regardless of the underlying OS's specific error messaging.

**Assumptions**: The code references methods `Self::ensure_name_allowed`, `Self::path_for_handle`, `Self::wcc_attr_from_metadata`, `Self::io_error_to_vfs`, `Self::metadata`, `Self::attr_from_metadata`, and `Self::wcc_data`. These methods are not defined in the provided `symlink_impl.rs` file nor in the `facts.json` for `mirrorfs::fs`. The specification assumes these methods exist on `MirrorFS` (likely in `mod.rs`) and function as their names suggest (validation, path conversion, metadata mapping, and error mapping).