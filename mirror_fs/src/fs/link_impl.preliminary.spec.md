<!-- SPEC_HASH: 3e1cd06f6c9a7d18dfdf4e0935fafdc94bd8cd0d2c32c2d25defe267bf30b3e1 -->
# Module Specification

Module: mirrorfs::fs::link_impl
Rust File: mirror_fs/src/fs/link_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`tokio::fs`**: Used to perform the asynchronous filesystem operation `fs::hard_link`. This is the core mechanism that creates the new directory entry (hard link) on the underlying operating system.
- **`nfs_mamont::vfs`**: Used to import the `vfs` module contents, specifically `vfs::Error` and `vfs::WccData`. These types are required to construct the return values (`Success` and `Fail`) defined by the `Link` trait.
- **`nfs_mamont::vfs::file`**: Used to import `file` module contents, specifically `file::Type` (to check if the source is a directory) and `file::Attr` (to return file attributes).
- **`nfs_mamont::vfs::link`**: Used to import the `link` module traits and structs (`Link`, `Args`, `Success`, `Fail`) which define the interface this module implements.
- **`super::MirrorFS`**: The parent struct for which this implementation is provided. The implementation relies on several helper methods assumed to exist on `MirrorFS` (e.g., `path_for_handle`, `ensure_name_allowed`, `file_attr`, `wcc_data`) to bridge VFS handles with filesystem paths.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a concrete implementation of the NFSv3 `LINK` procedure for the `MirrorFS` backend.
- To translate abstract VFS handles and names into concrete filesystem paths, perform a hard link operation, and manage the resulting state changes (attributes and cache consistency data).
- To enforce specific constraints, such as preventing hard links to directories, which are disallowed in many contexts and handled as `InvalidArgument`.

Inputs:
- **`args: link::Args`**: Contains the source file handle (`args.file`) and the target directory location (`args.link`), which includes the directory handle and the new link name.

Outputs:
- **`Result<link::Success, link::Fail>`**:
 - **`Success`**: Contains the post-operation attributes of the source file and the Weak Cache Consistency (WCC) data for the target directory.
 - **`Fail`**: Contains the error that occurred, the attributes of the source file (if available), and the WCC data for the target directory.

Steps:
1. **Name Validation**: The method calls `Self::ensure_name_allowed` with the new link name. If this check fails (e.g., illegal characters), it returns `Fail` immediately with `None` for attributes and WCC data.
2. **Source Path Resolution**: It calls `self.path_for_handle(&args.file).await` to convert the source file handle into a filesystem path. If this fails, it returns `Fail`.
3. **Source Type Check**: It retrieves the attributes of the source file using `Self::file_attr`. It checks if the file type is `Directory`. If it is, the method returns `Fail` with `vfs::Error::InvalidArgument`, as hard linking directories is not permitted.
4. **Target Directory Resolution**: It calls `self.path_for_handle(&args.link.dir).await` to resolve the target directory handle to a path. If this fails, it returns `Fail`.
5. **Pre-Operation State Capture**: It captures the metadata of the target directory *before* modification using `std::fs::symlink_metadata` and converts it to `WccAttr` via `Self::wcc_attr_from_metadata`. This is stored in `before`.
6. **Link Creation**: It constructs the full target path by pushing the new name onto the directory path. It then awaits `fs::hard_link(&file_path, &target_path)`.
7. **Error Handling**: If the hard link operation fails, it maps the `io::Error` to a `vfs::Error` using `Self::io_error_to_vfs` and returns `Fail`, including the captured `before` state in `dir_wcc`.
8. **Cache Update**: On success, it calls `self.handle_for_path(&target_path).await`. The result is discarded (`let _`), implying this is a cache-warming or internal mapping update step that does not affect the return value.
9. **Success Response**: It returns `Success`, populating `file_attr` with the attributes of the source file and `dir_wcc` with the full `WccData` (constructed using `Self::wcc_data` with the `before` state).

Edge Cases:
- **Source is Directory**: The code explicitly checks `matches!(file_attr.as_ref().map(|attr| attr.file_type), Some(file::Type::Directory))` and returns `InvalidArgument`.
- **Handle Resolution Failure**: If either the source file or target directory handles cannot be resolved to paths, the operation fails immediately without attempting the link.
- **Name Validation Failure**: If the link name is rejected by `ensure_name_allowed`, the operation fails early.

Complexity:
- **Time**: Dominated by filesystem I/O operations: `path_for_handle` (likely involves metadata lookups), `symlink_metadata`, and `hard_link`. Complexity is dependent on the OS filesystem latency.
- **Space**: O(1) additional stack space for paths and metadata structures.

Determinism:
- **Deterministic**: Given the same state of the filesystem and handle mappings, the operations will produce the same result.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::link`**:
 - **`Link` Trait**: Defines the signature `async fn link(&self, args: Args) -> Result<Success, Fail>`. This module implements this trait for `MirrorFS`.
 - **`Args`, `Success`, `Fail`**: These structs define the data contract. The implementation must populate `Success` with `file_attr` and `dir_wcc`, and `Fail` with `error`, `file_attr`, and `dir_wcc`.

- **From `nfs_mamont::vfs::file`**:
 - **`Type` Enum**: The variant `Type::Directory` is used to detect if the source file is a directory, triggering an `InvalidArgument` error.
 - **`Attr` Struct**: The return type of `Self::file_attr`, used to populate the `file_attr` field in the response.

- **From `nfs_mamont::vfs`**:
 - **`Error` Enum**: Used to wrap failures (e.g., `InvalidArgument`, IO errors mapped via `io_error_to_vfs`) for return in the `Fail` struct.
 - **`WccData` Struct**: Used to wrap the `before` and `after` state of the directory. The implementation uses `Self::wcc_data` (assumed helper) to construct this.

- **From `tokio::fs`**:
 - **`hard_link` Function**: The asynchronous primitive used to create the hard link on the disk.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It implements logic for `MirrorFS` using types defined in dependencies.

Relations:
- **`MirrorFS` implements `link::Link`**: This module provides the implementation body for the `link` method required by the trait.

Global Invariants:
- **Directory Link Restriction**: The implementation enforces that a hard link cannot be created if the source file is a directory.
- **WCC Data Availability**: The implementation attempts to provide `WccData` (specifically the `before` state) even in failure cases, provided the directory path was successfully resolved.

---

## 5. Error Model

Error Types:
- **`link::Fail`**: The wrapper struct returned on failure, containing a `vfs::Error`.
- **`vfs::Error`**: The specific error codes returned include:
 - `InvalidArgument`: Returned if the source file is a directory or if the link name is invalid (via `ensure_name_allowed`).
 - IO-related errors (e.g., `NoEntry`, `IO`, `Exist`): Returned via `Self::io_error_to_vfs` if the `fs::hard_link` call fails.

Error Propagation Strategy:
- **Early Return**: The function uses a series of `if let Err(error)` or `match` blocks. If any validation or resolution step fails, it returns `link::Fail` immediately.
- **Context Preservation**: When returning `Fail`, the code attempts to include `file_attr` (if already fetched) and `dir_wcc` (if the directory path was resolved) to provide the client with cache consistency data despite the failure.

Recoverability:
- **Client-Side**: Recoverability depends on the specific `vfs::Error` returned. For example, `InvalidArgument` implies a client error in the request, while `IO` might be transient.

Panics:
- **Allowed**: No explicit panics are introduced in this code. All error paths are handled via `Result` returns.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::link::Link`**: Implemented for `MirrorFS`.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependencies. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than this module.

This module is used in order to **implement the filesystem backend logic for creating hard links** within the `MirrorFS` driver. The system contains a complex architecture where the `nfs_mamont` crate defines the generic NFSv3 protocol interfaces (traits), and `mirror_fs` provides a concrete implementation backed by the local operating system's filesystem. This module is necessary because the generic `Link` trait only defines *what* a link operation looks like (handles, arguments, return values), whereas this module defines *how* that operation is performed specifically for a mirrored local filesystem—translating handles to paths, invoking async syscalls, and managing the specific constraints of the local FS.

A typical usage scenario of the system involves an NFS client sending a `LINK` request to create an additional name for an existing file. The request reaches the `MirrorFS` handler. This module takes the abstract file handles provided in the request, resolves them to absolute paths on the local disk (e.g., `/mnt/export/file1`), checks if the source is a directory (to prevent invalid operations), and then calls `tokio::fs::hard_link` to create the entry. It also captures the directory's metadata before and after the operation to construct the `WccData` required by the NFS protocol to keep client caches consistent.

Inside the system, the following things happen and they use this module:
1.  **Protocol-to-OS Translation**: The module acts as the bridge between the VFS abstraction (which uses opaque `Handle` types) and the OS reality (which uses `PathBuf`). It relies on `MirrorFS`'s internal mapping (via `path_for_handle`) to perform this translation.
2.  **Constraint Enforcement**: The NFS protocol has specific rules about hard links (e.g., no cross-device links, no linking directories). While the `Link` trait documents these, this module enforces the "no directory links" rule explicitly by checking `file::Type::Directory` before attempting the syscall.
3.  **Cache Consistency Management**: The module ensures that the client receives `WccData` (Weak Cache Consistency data) by capturing the directory's metadata *before* the link creation attempt. If the link fails, this data is still returned, allowing the client to validate its cache of the directory contents even if the operation didn't succeed.

**Uncertainty**: The code relies on several helper methods of `MirrorFS` (`ensure_name_allowed`, `path_for_handle`, `file_attr`, `wcc_attr_from_metadata`, `wcc_data`, `io_error_to_vfs`) which are not listed in the provided `*.facts.json` for `mirrorfs::fs`. The analysis assumes these methods exist as internal implementation details of `MirrorFS` to handle path resolution, attribute fetching, and error mapping.