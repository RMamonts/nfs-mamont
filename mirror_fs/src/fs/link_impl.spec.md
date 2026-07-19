<!-- SPEC_HASH: 3e1cd06f6c9a7d18dfdf4e0935fafdc94bd8cd0d2c32c2d25defe267bf30b3e1 -->
# Module Specification

Module: mirrorfs::fs::link_impl
Rust File: mirror_fs/src/fs/link_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs`**: Used to perform the asynchronous filesystem operation `hard_link`, which creates the actual hard link on the underlying storage.
- **`nfs_mamont::vfs`**: Used to import the `vfs` module structure, specifically `vfs::Error` for error mapping and `vfs::WccData` for constructing the Weak Cache Consistency data returned to the client.
- **`nfs_mamont::vfs::file`**: Used to import `file` module types like `file::Type` (to check if the source is a directory) and `file::Attr` (to hold file attributes).
- **`nfs_mamont::vfs::link`**: Used to import the `Link` trait and its associated types (`Args`, `Success`, `Fail`). This module provides the implementation of this trait for the `MirrorFS` struct.
- **`super::MirrorFS`**: The parent struct for which the `Link` trait is being implemented. The implementation relies on several helper methods assumed to be defined on `MirrorFS` (e.g., `path_for_handle`, `file_attr`, `ensure_name_allowed`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement the `nfs_mamont::vfs::link::Link` trait for the `MirrorFS` backend, enabling the creation of hard links within the mirrored filesystem.
- To translate abstract VFS handles into concrete filesystem paths, perform the link operation, and gather the necessary metadata (attributes and WCC data) required by the NFSv3 protocol.

Inputs:
- **`args: link::Args`**: Contains the `file` handle (source) and `link` (target directory handle and new name).

Outputs:
- **`Result<link::Success, link::Fail>`**: Returns a structure containing the post-operation attributes of the source file and the WCC data for the target directory, or an error structure containing the failure reason and available metadata.

Steps:
1. **Name Validation**: Calls `Self::ensure_name_allowed` with the new link name. If validation fails, returns `link::Fail` immediately with the specific error, no file attributes, and empty WCC data.
2. **Source Path Resolution**: Calls `self.path_for_handle(&args.file)` to convert the source file handle into a filesystem path. If this fails, returns `link::Fail` with the error.
3. **Source Attribute Retrieval**: Calls `Self::file_attr(&file_path)` to get the attributes of the source file.
4. **Directory Check**: Checks if the source file type is `Directory`. If it is, returns `link::Fail` with `vfs::Error::InvalidArgument`, as hard linking directories is not permitted.
5. **Target Directory Resolution**: Calls `self.path_for_handle(&args.link.dir)` to convert the target directory handle into a filesystem path. If this fails, returns `link::Fail` including the source file attributes if they were successfully retrieved.
6. **Pre-Operation State Capture**: Captures the metadata of the target directory using `std::fs::symlink_metadata` to establish the "before" state for WCC. Converts this metadata into `WccAttr` using `Self::wcc_attr_from_metadata`.
7. **Target Path Construction**: Constructs the full path for the new link by joining the target directory path with the new link name.
8. **Hard Link Creation**: Invokes `fs::hard_link(&file_path, &target_path).await`.
   - If successful, it updates the internal handle cache by calling `self.handle_for_path(&target_path).await` (the result is discarded).
   - If it fails, the IO error is converted to a `vfs::Error` using `Self::io_error_to_vfs`, and `link::Fail` is returned with the error, source attributes, and the calculated WCC data.
9. **Success Response**: Returns `link::Success` containing the source file attributes and the WCC data for the directory (calculated using the "before" state captured in step 6).

Edge Cases:
- **Source is Directory**: The implementation explicitly prevents hard linking to directories, returning `InvalidArgument`.
- **Name Validation Failure**: If the name is invalid (e.g., contains illegal characters), the operation fails before any filesystem path resolution or IO occurs.
- **Partial Metadata**: If the source file attributes are retrieved but the target directory resolution fails, the error response includes the source file attributes but empty WCC data (since the directory path was unknown).

Complexity:
- **Time**: O(N) where N is the depth of the directory tree (due to path resolution and metadata lookups). The `hard_link` syscall itself is generally O(1) or dependent on the filesystem implementation.
- **Space**: O(1) additional space, excluding the allocation of path strings.

Determinism:
- **Deterministic**: Given the same filesystem state and inputs, the sequence of checks and the resulting success/failure are deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::link`**:
 - **`Link` Trait**: Defines the `link` method signature and the contract for creating hard links. This module fulfills this contract.
 - **`Args`, `Success`, `Fail`**: The data structures used to transport inputs and outputs. The implementation populates `Success` with `file_attr` and `dir_wcc`, and `Fail` with `error`, `file_attr`, and `dir_wcc`.

- **From `nfs_mamont::vfs::file`**:
 - **`Type`**: Used to check the `file_type` field of the source file attributes to ensure it is not a `Directory`.
 - **`Attr`**: Used to hold the metadata of the source file that is returned in both `Success` and `Fail` responses.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: Used to structure the "before" and "after" attributes of the target directory. The implementation calculates the "before" state explicitly and relies on `Self::wcc_data` to finalize the structure.

- **From `super::MirrorFS` (Assumed)**:
 - **`path_for_handle`**: Critical for translating the abstract VFS handles (source file and target directory) into concrete filesystem paths required by `tokio::fs::hard_link`.
 - **`ensure_name_allowed`**: Provides the validation logic for the new link name, ensuring it meets system or protocol constraints.
 - **`file_attr`**: Retrieves the file metadata from the disk.
 - **`io_error_to_vfs`**: Maps standard IO errors (e.g., `NotFound`, `PermissionDenied`) to the specific `vfs::Error` codes required by the NFS protocol.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on `Args`, `Success`, and `Fail` defined in `nfs_mamont::vfs::link`.

Relations:
- **`MirrorFS` implements `Link`**: The module provides the concrete implementation logic for the trait defined in the dependency.

Global Invariants:
- **Directory Exclusion**: The source file for a hard link must not be a directory.
- **Name Validity**: The link name must pass the `ensure_name_allowed` check before any filesystem modification is attempted.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped inside `link::Fail`. Specific errors returned include:
 - `InvalidArgument`: If the source is a directory or the name is invalid.
 - IO-related errors (mapped via `io_error_to_vfs`): If path resolution or the hard link syscall fails.

Error Propagation Strategy:
- **Early Return**: The function uses a strategy of early returns on failure. If any step fails (validation, resolution, linking), it immediately constructs a `link::Fail` struct with the available information (e.g., file attributes if they were already fetched) and returns it.

Recoverability:
- **Client-Side**: Recoverability depends on the specific error code returned. `InvalidArgument` implies a client error in the request. IO errors might be transient.

Panics:
- **Allowed**: No. The code handles errors explicitly via `Result` types and `match`/`if let` statements.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::link::Link`**: Implemented for `super::MirrorFS`.

List which traits this module defines:
- None.

---

## 7. Overview

This module is used in order to **provide the concrete filesystem implementation for the NFSv3 `LINK` procedure** within the `MirrorFS` backend. The system contains a complex architecture where high-level NFS requests (defined by traits in `nfs_mamont::vfs`) must be translated into low-level filesystem operations (handled by `MirrorFS`). This module is necessary because it bridges the gap between the abstract VFS protocol (which deals in opaque handles and specific error codes) and the concrete reality of the local filesystem (which deals in paths, syscalls, and IO errors).

A typical usage scenario of the system involves an NFS client requesting a hard link. The request arrives as a `link::Args` struct containing handles. The `MirrorFS` backend delegates to this implementation. Here, the handles are resolved to actual file paths. The system verifies that the source is not a directory and that the name is valid. It then attempts to create the hard link using `tokio::fs`. Regardless of success or failure, it gathers the necessary metadata (file attributes and directory WCC data) to construct a compliant NFSv3 response (`Success` or `Fail`).

Inside the system, the following things happen and they use this module:
1. **Protocol Enforcement**: The module enforces the rule that directories cannot be hard links, returning `InvalidArgument` if the source is a directory. This ensures the backend behaves according to standard filesystem semantics expected by NFS clients.
2. **State Synchronization**: By calling `self.handle_for_path` after a successful link, the module ensures that the internal mapping of file handles to paths is updated. This is crucial for subsequent operations on the new link to resolve correctly.
3. **Error Translation**: The module uses `Self::io_error_to_vfs` to convert raw Rust IO errors into the standardized `vfs::Error` enum. This centralizes error handling logic, ensuring that the client receives correct NFS status codes (like `NFS3ERR_IO` or `NFS3ERR_ACCES`) rather than implementation-specific error strings.

**Uncertainty**: The specification for `super::MirrorFS` was not provided in the context. The analysis assumes the existence and behavior of methods `ensure_name_allowed`, `path_for_handle`, `file_attr`, `wcc_attr_from_metadata`, `wcc_data`, `io_error_to_vfs`, and `handle_for_path` based on their usage within this module. Their exact implementation details (e.g., how `path_for_handle` resolves a handle) are treated as opaque dependencies.