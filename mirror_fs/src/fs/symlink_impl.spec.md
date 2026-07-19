<!-- SPEC_HASH: 7fb3c54f8d601ff52e92286d5ebf79b07b1b80a066393492c882a0cb99f35d6c -->
# Module Specification

Module: mirrorfs::fs::symlink_impl
Rust File: src/fs/symlink_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::symlink`**: Used to implement the `Symlink` trait. This trait defines the contract for the NFSv3 `SYMLINK` procedure, specifying the input arguments (`Args`) and the expected success/failure structures (`Success`, `Fail`).
- **`nfs_mamont::vfs`**: Used to import the `WccData` structure, which is required to return Weak Cache Consistency data (pre- and post-operation attributes) for the directory where the link is created.
- **`std::os::unix::fs`**: Used to access the `symlink` function, which performs the actual OS-level system call to create a symbolic link on Unix-like systems.
- **`std::fs`**: Used to access `symlink_metadata`, which retrieves the metadata of the directory before the operation to populate the `before` field of `WccData`.
- **`super::MirrorFS`**: The parent struct for which this trait is implemented. The implementation relies on several helper methods assumed to exist on `MirrorFS` (see Assumptions in Mechanics) to translate between VFS handles and filesystem paths.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Symlink Creation with Weak Cache Consistency (WCC) Tracking

**Intent:**
To implement the NFSv3 `SYMLINK` procedure for a local filesystem mirror. This involves translating abstract VFS handles into concrete filesystem paths, performing the symlink creation, and collecting the necessary metadata to satisfy the NFS Weak Cache Consistency requirements.

**Inputs:**
- `args: symlink::Args`: Contains the target directory handle, the new link name, the target path content, and initial attributes (though initial attributes are not explicitly applied in the provided code snippet, they are part of the input).

**Outputs:**
- `Result<symlink::Success, symlink::Fail>`: On success, returns the new file handle, attributes, and directory WCC data. On failure, returns the error and directory WCC data.

**Steps:**
1. **Name Validation**: Calls `Self::ensure_name_allowed` with the new link name. If the name is invalid (e.g., contains disallowed characters), returns `symlink::Fail` immediately with empty WCC data.
2. **Path Resolution**: Calls `self.path_for_handle` to convert the directory handle from `args.object.dir` into a concrete filesystem path. If this fails, returns `symlink::Fail` with empty WCC data.
3. **Pre-Operation Snapshot**: Calls `std::fs::symlink_metadata` on the directory path to capture its state *before* modification. This is stored as `before`.
4. **Link Construction**: Constructs the full path for the new link by appending the link name to the directory path.
5. **System Call**: Invokes `std::os::unix::fs::symlink` with the target path and the new link path.
 - If this fails, maps the `std::io::Error` to a `vfs::Error` using `Self::io_error_to_vfs` and returns `symlink::Fail` containing the error and the WCC data (calculated from the `before` snapshot and a fresh metadata read).
6. **Post-Operation Metadata**: Calls `Self::metadata` on the new link path to get its attributes. If this fails, returns `symlink::Fail` with the error and directory WCC data.
7. **Handle Generation**: Calls `self.handle_for_path` to generate a VFS handle for the newly created link. If this fails, returns `symlink::Fail` with the error and directory WCC data.
8. **Success Response**: Returns `symlink::Success` containing the new handle, attributes, and the directory WCC data (calculated from `before` and current directory metadata).

**Edge Cases:**
- **Invalid Names**: If `ensure_name_allowed` fails, the operation aborts before any filesystem interaction.
- **Handle Resolution Failure**: If the directory handle cannot be resolved to a path, the operation fails without attempting to modify the filesystem.
- **Symlink Creation Failure**: If the OS `symlink` call fails (e.g., permission denied, disk full), the error is mapped to a VFS error, and the WCC data is still provided to the client.

**Complexity:**
- Time: O(1) amortized, dominated by filesystem syscalls (`stat`, `symlink`).
- Space: O(1) for path manipulation and metadata structures.

**Determinism:**
- Non-deterministic. Depends on the state of the underlying filesystem (permissions, disk space, existing files).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::symlink`**:
 - **`Symlink` Trait**: This module implements the `symlink` method defined by this trait. The trait dictates the requirement to return `WccData` and specific error types, which drives the logic in this implementation.
 - **`Args`, `Success`, `Fail`**: These structures define the shape of the data flowing into and out of the method.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: The implementation explicitly constructs this structure (via `Self::wcc_data`) to report the state of the parent directory, adhering to the NFSv3 caching model.

- **From `mirrorfs::fs::mod` (Assumptions)**:
 - **`MirrorFS`**: The implementation relies on several helper methods on `MirrorFS` which are not defined in the provided snippet but are inferred from usage:
 - `ensure_name_allowed(&str) -> Result<(), vfs::Error>`: Validates the filename.
 - `path_for_handle(&Handle) -> Result<PathBuf, vfs::Error>`: Converts VFS handles to paths.
 - `handle_for_path(&Path) -> Result<Handle, vfs::Error>`: Converts paths to VFS handles.
 - `metadata(&Path) -> Result<Metadata, vfs::Error>`: Reads file metadata.
 - `wcc_attr_from_metadata(&Metadata) -> WccAttr`: Converts OS metadata to VFS WCC attributes.
 - `attr_from_metadata(&Metadata) -> Attr`: Converts OS metadata to VFS attributes.
 - `wcc_data(&Path, Option<WccAttr>) -> WccData`: Constructs the final WCC structure.
 - `io_error_to_vfs(&io::Error) -> vfs::Error`: Maps standard IO errors to VFS errors.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on entities defined in `nfs_mamont::vfs::symlink` and `nfs_mamont::vfs`.

Relations:
- **Implementation**: `MirrorFS` implements `symlink::Symlink`.

Global Invariants:
- **WCC Consistency**: If the operation fails after the directory path is successfully resolved, the `Fail` structure must contain valid `dir_wcc` data (reflecting the directory state) rather than `None`, provided the directory metadata could be read.

## 5. Error Model

Error Types:
- **`symlink::Fail`**: The wrapper error type returned by the trait implementation. It contains a `vfs::Error` and `dir_wcc`.

Error Propagation Strategy:
- **Early Return**: The function uses a series of `match` or `if let` statements to check results of helper calls. If any step fails (validation, path resolution, symlink creation, metadata read, handle generation), it immediately returns a `symlink::Fail` struct.
- **Error Mapping**: OS-level `std::io::Error` from the `symlink` function is converted to `vfs::Error` using `Self::io_error_to_vfs`.

Recoverability:
- **Dependent on `vfs::Error`**: The recoverability is determined by the specific error code returned (e.g., `IO` might be transient, `Permission` requires user intervention).

Panics:
- **Allowed**: No. The code handles all potential errors (IO, validation) via `Result` returns.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::symlink::Symlink`**: Implemented for `MirrorFS`.

---

## 7. Overview

This module is used in order to **provide a concrete implementation of the NFSv3 `SYMLINK` procedure** for the `MirrorFS` backend. The `MirrorFS` system acts as a bridge between the abstract NFS protocol (which uses opaque handles and specific error codes) and a concrete local filesystem (which uses paths and standard OS errors). This module is necessary because the generic VFS layer defines *what* a symlink operation looks like, but the `MirrorFS` must define *how* to execute it on the actual disk.

The system contains a complex architecture where file system operations are abstracted behind traits. This module handles the specific logic required to create a symbolic link on a Unix-like filesystem while satisfying the strict requirements of the NFS protocol, specifically regarding Weak Cache Consistency (WCC). WCC requires the server to return the attributes of the directory *before* and *after* the modification so the client can validate its cache.

A typical usage scenario of the system involves an NFS client sending a `SYMLINK` request. The request is decoded into `symlink::Args` containing a directory handle and a link name. The `MirrorFS` implementation takes these arguments, resolves the handle to a real path (e.g., `/mnt/data`), checks if the name is allowed, and then calls the OS `symlink` function. It then reads the resulting metadata, generates a new handle for the link, and packages the directory's before/after attributes into `WccData` to return to the client.

Inside the system, the following things happen and they use this module:
1. **Handle Translation**: The module relies on `path_for_handle` and `handle_for_path` to translate between the NFS world (handles) and the OS world (paths). This is critical because `MirrorFS` maintains its own mapping of handles to inodes or paths.
2. **Atomicity Simulation**: While the code uses standard OS calls, the logic ensures that if the OS `symlink` call succeeds, the subsequent metadata and handle generation must also succeed; otherwise, a failure is returned, though the link already exists on disk. This highlights a limitation where the implementation cannot fully rollback the OS operation if post-creation steps fail, though it reports the failure to the client.
3. **WCC Calculation**: The module explicitly captures directory metadata *before* attempting the symlink creation. This ensures that even if the creation fails, the client receives the `before` attributes, which is required for cache coherency in NFSv3.

**Uncertainty**: The specification for `mirrorfs::fs::mod` was not provided, so the existence and exact signatures of helper methods like `ensure_name_allowed`, `path_for_handle`, `handle_for_path`, `wcc_data`, and `io_error_to_vfs` are inferred from their usage in this module. It is assumed they handle the internal logic of `MirrorFS` path and handle management.