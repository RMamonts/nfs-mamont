<!-- SPEC_HASH: f6ad4fd317d94d28f413c5fae549b70b042421fd4bb5a5ba8912e4e2bd30ec18 -->
# Module Specification

Module: nfs_mamont::vfs::fs_stat
Rust File: src/vfs/fs_stat.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::vfs**: Used to import the `Error` enum. This enum is utilized within the `Fail` struct to categorize and report specific failure conditions (e.g., permission issues, I/O errors) that occur when attempting to retrieve file system statistics.
- **super::file**: Used to import `file::Attr` and `file::Handle`. `Handle` is required in `Args` to identify the specific file system instance (mount point) being queried. `Attr` is used in both `Success` and `Fail` structs to return the attributes of the root directory, providing metadata alongside the statistics.
- **trait_variant**: Used via the `#[trait_variant::make(Send)]` attribute macro. This is necessary to transform the `FsStat` trait into an object-safe trait that also implements `Send`, allowing instances of the trait to be passed between threads in an asynchronous context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface contract for the NFSv3 `FSSTAT` procedure, which retrieves dynamic state information about a file system (such as total space, free space, and file slot counts).
- To structure the response data (`Success`) and error data (`Fail`) to match the semantics of the NFSv3 protocol, specifically distinguishing between total free space and space available to the user (accounting for reserved space).

Inputs:
- `Args`: A structure containing a `file::Handle` that identifies the root of the file system or the specific mount point to be queried.

Outputs:
- `Result<Success, Fail>`:
  - `Success`: Contains file system capacity metrics (bytes and file slots), volatility information (`invarsec`), and the attributes of the root handle.
  - `Fail`: Contains a `vfs::Error` describing the failure and, optionally, the attributes of the root handle (for weak cache consistency).

Steps:
1. **Interface Definition**: The module defines the `FsStat` trait, which requires an asynchronous method `fs_stat`.
2. **Argument Handling**: The method accepts `Args`, wrapping a `file::Handle`. This handle serves as the key to locate the specific file system context.
3. **Execution (Abstract)**: The implementation of this trait (defined elsewhere) is expected to query the underlying storage for current usage statistics.
4. **Result Construction**:
   - On success, the implementation populates `Success` with `total_bytes`, `free_bytes`, `available_bytes`, `total_files`, `free_files`, `available_files`, and `invarsec`. It also includes `root_attr` if available.
   - On failure, the implementation populates `Fail` with the specific `vfs::Error` and potentially `root_attr`.

Edge Cases:
- **Reserved Space**: The `Success` struct explicitly differentiates between `free_bytes` (total free on disk) and `available_bytes` (free space available to the user, excluding system-reserved space). The same logic applies to file slots.
- **Attribute Availability**: Both `Success` and `Fail` wrap `root_attr` in an `Option`. This implies that in certain failure modes or specific implementation details, the attributes of the root might not be retrievable or relevant.

Complexity:
- Time: Not defined in this module (depends on the implementor of the trait).
- Space: O(1) for the data structures defined (fixed size fields).

Determinism:
- Deterministic (Interface definition).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs::file`**:
  - **Handle**: Acts as the unique identifier for the file system root. The `FsStat` operation relies on this handle to target the correct file system instance.
  - **Attr**: Provides the metadata structure for the root directory. This is included in the response to allow the client to update its cache for the root directory attributes without making a separate `GETATTR` call.
- **From `crate::vfs` (mod.rs)**:
  - **Error**: Provides the enumeration of possible error conditions (e.g., `IO`, `StaleFile`, `Access`). The `Fail` struct uses this to signal to the NFS client why the `FSSTAT` operation could not be completed.

---

## 4. Data Model

Entities:
- **Args**: A message structure containing a `file::Handle` (`root`) identifying the mount point.
- **Success**: A message structure containing file system statistics.
  - `total_bytes`: Total capacity of the file system.
  - `free_bytes`: Total free space on the file system.
  - `available_bytes`: Free space available to the user (total free minus reserved).
  - `total_files`: Total file slots (inodes).
  - `free_files`: Total free file slots.
  - `available_files`: Free file slots available to the user.
  - `invarsec`: A measure of file system volatility (cache consistency hint).
  - `root_attr`: Optional attributes of the root handle.
- **Fail**: A message structure containing error information.
  - `error`: The specific `vfs::Error` that occurred.
  - `root_attr`: Optional attributes of the root handle.

Relations:
- **Args** uses `file::Handle`.
- **Success** and `Fail** both contain `Option<file::Attr>`.
- **Fail** contains `vfs::Error`.

Global Invariants:
- **Space Hierarchy**: In a valid `Success` struct, `available_bytes` must be less than or equal to `free_bytes`, and `available_files` must be less than or equal to `free_files`, as "available" implies a subset of "free" after accounting for reservations.

## 5. Error Model

Error Types:
- `vfs::Error`

Error Propagation Strategy:
- The module uses the `Result<Success, Fail>` pattern. Errors are not thrown as exceptions but returned within the `Fail` struct, which wraps the `vfs::Error` enum.

Recoverability:
- Recoverable. The caller receives the `Fail` struct and can inspect the `error` field to determine the next course of action (e.g., retry, abort, or notify the client).

Panics:
- Allowed: No
- Conditions: The module consists solely of struct definitions and a trait definition. There is no executable code in this module that could panic.

---

## 6. Traits

List which external traits this module implements:
- **Send**: Implemented for `FsStat` via the `#[trait_variant::make(Send)]` macro. This allows the trait object to be sent across threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the interface for retrieving volatile file system statistics, specifically corresponding to the `FSSTAT` procedure of the NFSv3 protocol. The system contains a high-performance NFS server that must translate abstract file system queries into concrete storage operations. A typical usage scenario of the system involves a client connecting to the server and requesting to know how much disk space is remaining on a specific exported share. The system uses the `FsStat` trait defined in this module to abstract this capability. The trait ensures that regardless of the underlying storage backend (whether it is a local disk, a networked block device, or an object store), the server can retrieve standardized metrics regarding total capacity, free space, and available file slots (inodes).

Inside the system, the following things happen and they use this module: The main request handler receives an RPC request for `FSSTAT`. It invokes the `fs_stat` method on the `Vfs` trait object (which aggregates `FsStat`). The implementation of this trait queries the backend for current usage. The `Success` struct is then populated with these metrics. Crucially, the module differentiates between "free" resources and "available" resources. This distinction is vital because many file systems reserve a portion of space (e.g., 5%) for the root user or system processes; reporting the total free space to a regular user might mislead them into thinking they can write more data than actually allowed. By providing `available_bytes` and `available_files`, the system accurately enforces these quotas at the protocol level. Furthermore, the inclusion of `root_attr` in both success and failure cases allows the client to perform weak cache consistency updates for the root directory of the mount point, optimizing subsequent operations by avoiding redundant `GETATTR` calls. Without this module, the VFS layer would lack a standardized mechanism to report these critical resource limits, leading to potential client-side errors (like "No space left on device" occurring unexpectedly) or inefficient cache management.