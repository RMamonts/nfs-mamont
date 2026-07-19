<!-- SPEC_HASH: 96e5933a34f175b4efb49867879554076425d688b128cbd774ef82022531fda9 -->
# Module Specification

Module: nfs_mamont::vfs::fs_info
Rust File: src/vfs/fs_info.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `Error` enum, which is wrapped in the `Fail` struct to report protocol or server errors. This module is also a sub-module of `vfs`, and its `FsInfo` trait is aggregated into the main `Vfs` super-trait defined in the parent.
- **`crate::vfs::file`**: Used to import core domain types: `Handle` (to identify the file system root in `Args`), `Attr` (to describe the root attributes in `Success` and `Fail`), and `Time` (to specify timestamp granularity in `Success`).
- **`super::file`**: An alias for `crate::vfs::file`, used to access the types defined in the sibling module.
- **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` attribute on the `FsInfo` trait. This macro generates an object-safe version of the async trait that implements `Send`, allowing the trait to be used as a trait object in dynamic contexts (e.g., `dyn FsInfo`) across thread boundaries.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `FSINFO` procedure, which allows clients to query static and dynamic parameters of the file system.
- To provide data structures that represent file system capabilities (properties), I/O performance limits (read/write sizes), and time granularity.
- To encapsulate the logic for handling file system property bitmasks, ensuring that only known flags are recognized when parsing data from the network.

Inputs:
- **`Args`**: A structure containing a `file::Handle` (`root`) that identifies the specific file system or mount point being queried.

Outputs:
- **`Result<Success, Fail>`**:
 - **`Success`**: Contains the file system attributes (`root_attr`), transfer size limits (`read_max`, `write_max`, etc.), preferred sizes, maximum file size, time granularity (`time_delta`), and supported properties (`properties`).
 - **`Fail`**: Contains a `vfs::Error` indicating the cause of failure and optionally the `root_attr` (for Weak Cache Consistency).

Steps:
1. **Property Masking**: When `Properties::from_wire` is called with a raw `u32` from the network, it applies a bitmask (`Self::ALL`) to zero out any undefined or reserved bits, ensuring internal consistency.
2. **Trait Invocation**: The `FsInfo::fs_info` method is called asynchronously. The implementer (the backend) is expected to look up the file system identified by `args.root` and populate the `Success` struct with the current parameters of that file system.
3. **Result Construction**: The backend returns either `Ok(Success)` with the populated parameters or `Err(Fail)` if the handle is invalid or an I/O error occurs.

Edge Cases:
- **Partial Failure Handling**: The `Fail` struct includes `root_attr: Option<file::Attr>`. This allows the server to return the attributes of the root even if the `FSINFO` operation itself failed, adhering to the NFSv3 recommendation to return pre- or post-operation attributes (WCC data) whenever possible to aid client cache consistency.
- **Unknown Property Bits**: The `Properties::from_wire` method silently ignores bits not present in `Self::ALL`. This prevents undefined flags from affecting internal logic.

Complexity:
- **Time**:
 - `Properties::from_wire`: O(1).
 - `FsInfo::fs_info`: Depends on the backend implementation, but typically O(1) or O(log N) depending on how the handle is resolved.
- **Space**: O(1) for all structures defined in this module.

Determinism:
- **Deterministic**: The interface definition and the bitmask logic are deterministic. The actual values returned in `Success` depend on the state of the backend file system.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: Used in `Args` to uniquely identify the file system instance. The `FsInfo` operation is scoped to the file system referred to by this handle.
 - **`Attr`**: Used in `Success` and `Fail` to provide the metadata of the root directory. This allows the client to verify the validity of the handle or update its cache.
 - **`Time`**: Used in `Success` to define `time_delta`. This is critical for clients to understand the precision with which they can set file times (e.g., via `SETATTR`).

- **From `nfs_mamont::vfs`**:
 - **`Error` Enum**: Used in `Fail` to standardize error reporting. If the file system handle is stale, the backend returns `vfs::Error::StaleFile`, which the RPC layer translates to the appropriate NFS status code.
 - **`Vfs` Trait**: The `FsInfo` trait defined in this module is a super-trait requirement of the main `vfs::Vfs` trait. This means any implementation of the VFS must support the `FSINFO` procedure.

---

## 4. Data Model

Entities:
- **`Properties`**: A newtype wrapper around `u32` representing a bitmask of file system capabilities.
 - Flags: `LINK`, `SYMLINK`, `HOMOGENEOUS`, `CANSETTIME`.
 - Invariants: The internal integer is always masked to contain only known bits when constructed via `from_wire`.
- **`Success`**: A struct containing the result of a successful `FSINFO` call.
 - Fields: `root_attr`, `read_max`, `read_pref`, `read_mult`, `write_max`, `write_pref`, `write_mult`, `read_dir_pref`, `max_file_size`, `time_delta`, `properties`.
- **`Fail`**: A struct containing the result of a failed `FSINFO` call.
 - Fields: `error`, `root_attr`.
- **`Args`**: Arguments passed to the `fs_info` method.
 - Fields: `root` (a `file::Handle`).

Relations:
- **Composition**: `Success` and `Fail` both optionally contain `file::Attr`.
- **Composition**: `Success` contains `Properties`.
- **Association**: `Args` contains a `file::Handle` which acts as the key to retrieve the information in `Success`.

Global Invariants:
- **Transfer Limits**: The values in `Success` (e.g., `read_max`, `write_max`) represent hard limits on the server. Clients must respect these limits to avoid short reads/writes or errors.
- **Time Granularity**: The `time_delta` in `Success` indicates the minimum resolution for timestamps. Clients should not expect timestamps to be more precise than this value.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type wrapped in the `Fail` struct. This enum covers standard NFS errors (e.g., `StaleFile`, `IO`) and server-specific errors.

Error Propagation Strategy:
- **Result Wrapper**: The `fs_info` method returns `Result<Success, Fail>`. The `Fail` struct wraps the `vfs::Error`, allowing the caller to distinguish between different failure modes while still potentially accessing `root_attr`.

Recoverability:
- **Dependent on Error**: If the error is `vfs::Error::IO`, the operation might be retried. If the error is `vfs::Error::StaleFile`, the client must re-lookup the file handle.

Panics:
- **Allowed**: No. The interface is designed to return errors via `Result`, not to panic.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`FsInfo`**: An asynchronous trait with the `Send` variant generated. It defines the `fs_info` method for retrieving file system information.

---

## 7. Overview

This module is used in order to **expose the capabilities and configuration limits of the file system to NFS clients**. The system contains a complex architecture where the client needs to optimize its behavior based on the server's constraints (e.g., buffer sizes, supported features). This module defines the contract (`FsInfo` trait) that the storage backend must implement to provide this information.

A typical usage scenario of the system involves a client mounting a file system and immediately issuing an `FSINFO` request. The RPC layer receives this request, extracts the file handle, and calls the `fs_info` method on the VFS backend. The backend inspects the configuration or state of the storage volume identified by the handle and returns a `Success` struct containing parameters like `rtmax` (maximum read size) and `wtmax` (maximum write size). The client then uses these values to size its I/O buffers for subsequent `READ` and `WRITE` operations, ensuring efficient data transfer without hitting server limits.

Inside the system, the following things happen and they use this module:
1. **Capability Discovery**: The `Properties` struct allows the server to tell the client whether it supports hard links (`LINK`), symbolic links (`SYMLINK`), or setting file times (`CANSETTIME`). The client uses this information to determine which NFS procedures are valid for this specific file system.
2. **Performance Tuning**: The fields `read_max`, `read_pref`, `read_mult`, etc., in the `Success` struct guide the client's I/O strategy. For example, if `read_mult` is 4096, the client knows that reading in multiples of 4096 bytes is most efficient for the underlying storage.
3. **Time Precision**: The `time_delta` field informs the client about the granularity of the server's clock. This is crucial for operations like `SETATTR` where the client attempts to set a file's modification time; the server will truncate the time to the precision specified by `time_delta`.

Without this module, the client would have no standard way to discover these parameters, leading to inefficient default buffer sizes or failed operations when attempting unsupported features (like creating symlinks on a file system that doesn't support them). This module ensures that the server can explicitly declare its behavior and limits.