<!-- SPEC_HASH: bf432572f5d5fdf50081bc44e8dc2031af032b53920cc5e9cde20bbec5e29736 -->
# Module Specification

Module: mirrorfs::fs::fs_info_impl
Rust File: mirror_fs/src/fs/fs_info_impl.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`nfs_mamont::vfs::fs_info`**: Used to import the `FsInfo` trait, which this module implements for the `MirrorFS` struct. It also provides the data structures `Args`, `Success`, `Fail`, and `Properties` required to define the input and output of the `fs_info` operation.
- **`nfs_mamont::vfs::file`**: Used to import the `Time` struct, which is instantiated to define the `time_delta` field in the `Success` response, indicating the server's timestamp granularity.
- **`super` (i.e., `mirrorfs::fs`)**: Used to import the `MirrorFS` struct definition and the constants `READ_DIR_PREF` and `READ_WRITE_MAX`. These constants define the I/O performance limits and preferences specific to this file system implementation.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide the concrete implementation of the NFSv3 `FSINFO` procedure for the `MirrorFS` backend.
- To report static file system capabilities (supported features, I/O limits) and dynamic root attributes to the client.
- To map the abstract file handle provided by the client to a concrete local path to retrieve current metadata.

Inputs:
- **`args: fs_info::Args`**: Contains the `root` file handle (`file::Handle`) identifying the file system instance being queried.

Outputs:
- **`Result<fs_info::Success, fs_info::Fail>`**:
 - **`Success`**: Contains the root attributes, transfer size limits, time granularity, and supported properties.
 - **`Fail`**: Contains a `vfs::Error` if the handle resolution fails.

Steps:
1. **Handle Resolution**: The method calls `self.path_for_handle(&args.root).await`.
 - *Assumption*: `path_for_handle` is an internal method of `MirrorFS` (or its parent module) that resolves a generic file handle to a specific local path. It is not listed in the public facts of `mirrorfs::fs` but is used here.
2. **Error Handling**: If `path_for_handle` returns an `Err`, the method immediately returns `fs_info::Fail` containing the error and `root_attr: None`.
3. **Attribute Retrieval**: If the handle is resolved successfully, the method calls `Self::file_attr(&path)`.
 - *Assumption*: `file_attr` is an internal helper method that retrieves the `file::Attr` for the given local path.
4. **Response Construction**: The method constructs and returns `fs_info::Success` with the following fields:
 - `root_attr`: Populated with the result of `Self::file_attr`.
 - `read_max`, `read_pref`, `write_max`, `write_pref`: Set to the constant `READ_WRITE_MAX`.
 - `read_mult`, `write_mult`: Set to `1`.
 - `read_dir_pref`: Set to the constant `READ_DIR_PREF`.
 - `max_file_size`: Set to `u64::MAX`.
 - `time_delta`: Set to `file::Time { seconds: 0, nanos: 1 }`, indicating nanosecond precision.
 - `properties`: Constructed using `fs_info::Properties::from_wire` with the flags `LINK`, `SYMLINK`, `HOMOGENEOUS`, and `CANSETTIME` enabled.

Edge Cases:
- **Invalid Handle**: If the provided `args.root` does not correspond to a valid path known to `MirrorFS`, `path_for_handle` is expected to return an error, which results in a `Fail` response.

Complexity:
- **Time**: Depends on the implementation of `path_for_handle` and `file_attr`. Assuming these are O(1) or O(log N) lookups, the complexity is effectively O(1) relative to the input size.
- **Space**: O(1) for the structures created.

Determinism:
- **Deterministic**: The output is fully determined by the state of the file system at the moment of the call (specifically the attributes of the root) and the hardcoded constants.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::fs_info`**:
 - **`FsInfo` Trait**: Defines the asynchronous interface `fs_info` that this module implements. It enforces the contract that the backend must accept `Args` and return a `Result<Success, Fail>`.
 - **`Properties::from_wire`**: Used to safely construct the properties bitmask. Although the flags are hardcoded here, this method ensures that only valid bits are set in the resulting `Properties` struct.

- **From `nfs_mamont::vfs::file`**:
 - **`Time`**: Used to construct the `time_delta` field. The specific values `seconds: 0, nanos: 1` indicate that the file system supports nanosecond granularity for timestamps.

---

## 4. Data Model

Entities:
- This module defines no new public entities. It operates on `fs_info::Args`, `fs_info::Success`, `fs_info::Fail`, and `file::Time`.

Relations:
- **Implementation**: `MirrorFS` implements `fs_info::FsInfo`.

Global Invariants:
- **Static Limits**: The values for `read_max`, `write_max`, and `max_file_size` are static constants defined in the parent module (`READ_WRITE_MAX` and `u64::MAX`). They do not change based on the state of the underlying storage.
- **Supported Features**: The file system always reports support for `LINK`, `SYMLINK`, `HOMOGENEOUS`, and `CANSETTIME`.

## 5. Error Model

Error Types:
- **`vfs::Error`**: Wrapped inside `fs_info::Fail`. This error originates from the `path_for_handle` method if the handle cannot be resolved.

Error Propagation Strategy:
- **Early Return**: If `path_for_handle` fails, the error is immediately wrapped in `fs_info::Fail` and returned. No `root_attr` is provided in the failure case (`root_attr: None`).

Recoverability:
- **Dependent on Error**: If the error is `StaleFile` or `BadHandle`, the client must re-lookup the file handle. If it is an `IO` error, the client might retry.

Panics:
- **Allowed**: No.
- **Conditions**: The implementation uses `match` and `return` for error handling, avoiding panics.

---

## 6. Traits

List which external traits this module implements:
- **`nfs_mamont::vfs::fs_info::FsInfo`**: Implemented for `MirrorFS`.

List which traits this module defines:
- None.

---

## 7. Overview

This module is used in order to **implement the capability discovery mechanism (`FSINFO`) for the MirrorFS backend**. The system contains a complex architecture where the NFS server must be able to describe the limits and features of the underlying storage to the client. This module is necessary because it connects the generic `FsInfo` trait defined in the `nfs_mamont` library with the specific implementation details of the `MirrorFS` file system (such as its specific I/O limits and handle resolution logic).

A typical usage scenario of the system involves a client mounting the MirrorFS share and issuing an `FSINFO` RPC request to determine how to size its read/write buffers and whether it can create symbolic links. The RPC layer receives the request, extracts the file handle, and calls the `fs_info` method implemented in this module. The module resolves the handle to a local path using internal logic (`path_for_handle`), retrieves the current attributes of the root (`file_attr`), and combines them with static configuration constants (`READ_WRITE_MAX`) to return a comprehensive `Success` struct to the client.

Inside the system, the following things happen and they use this module:
1. **Handle Validation**: The module uses `path_for_handle` to verify that the file handle provided by the client is valid and corresponds to an existing path in the mirrored file system. If the handle is invalid, it returns a `Fail` status.
2. **Capability Advertisement**: The module explicitly sets the `properties` field to indicate support for hard links (`LINK`), symbolic links (`SYMLINK`), and setting file times (`CANSETTIME`). This informs the client that it can safely use these NFS procedures.
3. **Performance Tuning**: By returning `READ_WRITE_MAX` in the `read_max` and `write_max` fields, the module instructs the client on the optimal block size to use for data transfer, ensuring efficient network utilization.

**Uncertainty**: The methods `path_for_handle` and `file_attr` are used in the code but are not listed in the provided public interface facts for `mirrorfs::fs`. It is assumed that `path_for_handle` is an internal method that performs the reverse operation of the public `handle_for_path` (resolving a handle to a path), and `file_attr` is a helper that retrieves file system attributes for a given path.