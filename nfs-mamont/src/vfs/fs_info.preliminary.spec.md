<!-- SPEC_HASH: 96e5933a34f175b4efb49867879554076425d688b128cbd774ef82022531fda9 -->
# Module Specification

Module: nfs_mamont::vfs::fs_info
Rust File: src/vfs/fs_info.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::vfs**: Used to import the `vfs::Error` enum for error reporting within the `Fail` struct and `vfs::file::Time` for representing time granularity in the `Success` struct.
- **super::file**: Used to import `file::Attr` for representing file system root attributes in both `Success` and `Fail` structs, and `file::Handle` for identifying the file system root in the `Args` struct.
- **trait_variant::make**: Used to transform the `FsInfo` trait into an object-safe trait that implements `Send`, allowing it to be used as a trait object across thread boundaries in an asynchronous context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface and data structures for the NFSv3 `FSINFO` procedure, which allows clients to query static or semi-static information about the server's file system.
- To provide type-safe bit manipulation for file system properties (e.g., support for symlinks, hard links) via the `Properties` struct.
- To encapsulate transfer size limits (read/write) and time granularity, enabling clients to optimize I/O operations and cache consistency strategies.

Inputs:
- `Args`: A structure containing a `file::Handle` representing the root of the file system or a specific mount point.
- `u32`: Raw bit values for constructing `Properties` from wire format.

Outputs:
- `Result<Success, Fail>`: The result of the `fs_info` operation.
- `Properties`: A sanitized bit mask of file system capabilities.
- `Success`: A structure containing file system attributes, transfer size limits, and properties.
- `Fail`: A structure containing an error code and optional root attributes.

Steps:
1. **Properties Sanitization**: When `Properties::from_wire` is called with a raw `u32`, the method applies a bitmask (`Self::ALL`) to ignore any undefined or reserved bits, ensuring only known flags (LINK, SYMLINK, HOMOGENEOUS, CANSETTIME) are set.
2. **Capability Reporting**: The `Success` struct aggregates various parameters:
   - Transfer sizes (`read_max`, `write_max`, etc.) inform the client of the server's buffer limits.
   - `time_delta` informs the client of the server's timestamp precision.
   - `properties` indicates which features (like symlinks) are supported.
3. **Interface Definition**: The `FsInfo` trait defines the asynchronous contract `fs_info` that implementations must fulfill. It takes `Args` and returns a `Result`.

Edge Cases:
- **Properties Masking**: If the input `u32` to `Properties::from_wire` contains bits outside the defined `ALL` mask, they are silently dropped rather than causing an error.
- **Optional Attributes**: The `root_attr` field in both `Success` and `Fail` is `Option<file::Attr>`, meaning the server may choose not to return attributes even on success or failure, though typically they are returned to aid cache consistency.

Complexity:
- Time: O(1) for all struct methods and data access. The async `fs_info` execution time depends on the specific VFS implementation.
- Space: O(1) for all defined structs (`Properties`, `Success`, `Fail`, `Args`).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs::file`**:
  - **Handle**: Used in `Args` to identify the target file system. The `Handle` is a fixed-size opaque byte array acting as a unique identifier.
  - **Attr**: Used in `Success` and `Fail` to provide metadata about the file system root. This struct aggregates type, mode, ownership, size, and timestamps.
  - **Time**: Used in `Success` (`time_delta`) to define the granularity of time stamps on the server.

- **From `crate::vfs`**:
  - **Error**: Used in `Fail` to report specific protocol or system errors (e.g., `IO`, `StaleFile`, `NotSupported`) that occurred during the operation.

---

## 4. Data Model

Entities:
- **Properties**: A wrapper around a `u32` acting as a bitset. Flags include `LINK` (hard links supported), `SYMLINK` (symbolic links supported), `HOMOGENEOUS` (all files have same properties), and `CANSETTIME` (server can set file times).
- **Success**: Represents a successful `FSINFO` response. Contains transfer size limits (`read_max`, `write_max`, etc.), preferred sizes, directory read preference, maximum file size, time delta, and the `Properties` bitmask.
- **Fail**: Represents a failed `FSINFO` response. Contains a `vfs::Error` and optionally the `root_attr`.
- **Args**: Represents the request arguments. Contains a `file::Handle` pointing to the file system root.

Relations:
- **Composition**: `Success` contains `Properties`.
- **Association**: `Success` and `Fail` optionally contain `file::Attr`.
- **Dependency**: `Args` contains `file::Handle`.

Global Invariants:
- **Properties Invariant**: The integer value wrapped by `Properties` will never have bits set outside of `Properties::ALL` if constructed via `from_wire`.

## 5. Error Model

Error Types:
- `vfs::Error`: An enumeration of NFSv3 protocol errors (e.g., `Permission`, `IO`, `StaleFile`, `NotSupported`).

Error Propagation Strategy:
- Errors are returned within the `Fail` struct variant of the `Result` returned by the `FsInfo::fs_info` method.

Recoverability:
- Recoverable. The caller receives the specific error code and can attempt to handle it (e.g., by retrying or aborting the operation).

Panics:
- Allowed: No
- Conditions: The code consists of struct definitions and a trait definition with no explicit panic points.

---

## 6. Traits

List which external traits this module implements:
- **Send**: Implemented for `FsInfo` via the `#[trait_variant::make(Send)]` attribute, allowing the trait object to be sent between threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the interface for retrieving static and dynamic parameters of a file system within the `nfs_mamont` NFSv3 server implementation. The system contains a complex Virtual File System (VFS) layer that abstracts the underlying storage (e.g., local disk, memory) from the NFS protocol logic. A typical usage scenario of the system involves an NFS client connecting to the server and issuing an `FSINFO` request immediately after mounting to discover the server's capabilities. The system uses the `FsInfo` trait defined in this module to query the VFS backend for parameters such as maximum read/write sizes, supported file system features (like symlinks or hard links), and time granularity.

Inside the system, the following things happen and they use this module: The NFS request handler receives an `FSINFO` RPC call containing a file handle. It constructs the `Args` struct with this handle and invokes the `fs_info` method on the VFS implementation. The VFS implementation (e.g., a POSIX wrapper) checks the underlying file system limits and capabilities, constructs a `Success` struct with these values, and returns it. The `Properties` struct ensures that the feature flags are correctly masked and interpreted according to the NFSv3 specification. Without this module, the server would lack a standardized way to communicate these critical parameters to the client, leading to potential inefficiencies (e.g., clients using buffer sizes too large for the server) or incorrect behavior (e.g., clients attempting to create symlinks on a file system that does not support them). This module bridges the gap between the generic NFS protocol requirements and the specific capabilities of the storage backend.