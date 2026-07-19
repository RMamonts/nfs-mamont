<!-- SPEC_HASH: b469951f56c2d476c2c606bad6612caf0592e08e3dabd956be8b44c4a3f4a02a -->
# Module Specification

Module: nfs_mamont::vfs::lookup
Rust File: src/vfs/lookup.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::vfs**: Used to import the `Error` enum. This enum provides the standardized error codes (e.g., `NoEntry`, `NotDir`, `Permission`) that the `lookup` operation must return to indicate specific failure conditions to the NFS client.
- **super::file**: Used to import the core data types `Handle`, `Name`, and `Attr`. These types define the input arguments (parent directory handle, filename) and the output payload (file handle, attributes) of the lookup operation, ensuring type safety and adherence to the NFSv3 protocol structure.
- **trait_variant**: Used to apply the `#[trait_variant::make(Send)]` attribute to the `Lookup` trait. This macro generates a version of the trait that is safe to send across threads, which is necessary for the trait to be used as an object in an asynchronous, multi-threaded server context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the asynchronous interface for the NFSv3 `LOOKUP` procedure within the VFS layer.
- To abstract the operation of resolving a file name within a specific directory context into a file handle.
- To enforce the contract that lookup operations do not follow symbolic links, returning the handle of the link itself instead.
- To support Weak Cache Consistency (WCC) by allowing the return of directory attributes even when the operation fails.

Inputs:
- `Args`: A struct containing:
  - `parent`: A `file::Handle` representing the directory to search.
  - `name`: A `file::Name` representing the entry to look up.

Outputs:
- `Result<Success, Fail>`:
  - `Success`: Contains the `file::Handle` of the found object, optional `file::Attr` of the object, and optional `file::Attr` of the parent directory (post-operation).
  - `Fail`: Contains a `vfs::Error` describing the failure and optional `file::Attr` of the parent directory (post-operation).

Steps:
1. The `lookup` method of the `Lookup` trait is invoked with a reference to `Args`.
2. The implementation attempts to locate the entry specified by `args.name` within the directory identified by `args.parent`.
3. If the entry is found and accessible:
   - A `Success` struct is created containing the file handle for the entry.
   - Attributes for the file and/or directory may be queried and included in the `Success` struct.
4. If the entry is not found, the parent is not a directory, or a permission error occurs:
   - A `Fail` struct is created containing the specific `vfs::Error`.
   - Post-operation attributes for the directory may still be queried and included in the `Fail` struct to allow the client to update its cache.

Edge Cases:
- **Symbolic Links**: If the target `name` corresponds to a symbolic link, the operation returns the handle for the symlink itself. It does not resolve the target path.
- **Attribute Availability**: The `file_attr` and `dir_attr` fields in `Success`, and `dir_attr` in `Fail`, are optional. The underlying implementation may choose not to fetch them if they are not requested or if fetching them is too costly, although NFSv3 generally expects them for cache consistency.

Complexity:
- Time: Dependent on the underlying file system implementation (typically O(N) for directory scanning where N is the number of entries in the directory, or O(1) for indexed/hashed directories).
- Space: O(1) for the data structures defined in this module (they hold handles and fixed-size attributes).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs::file`**:
  - **Handle**: Used as the unique identifier for both the input directory and the output file. It encapsulates the raw file handle bytes.
  - **Name**: Used to ensure that the `name` argument is a validated string (non-empty, no path separators) before the lookup operation proceeds.
  - **Attr**: Used to structure the metadata returned for the file and directory. This includes type, mode, size, and timestamps.
- **From `crate::vfs`**:
  - **Error**: Used to categorize failure modes. The `lookup` implementation maps internal file system errors (e.g., "not found", "permission denied") to specific variants of this enum.

---

## 4. Data Model

Entities:
- **Args**: A structure encapsulating the input for a lookup request.
  - `parent`: `file::Handle` (The directory context).
  - `name`: `file::Name` (The target filename).
- **Success**: A structure representing a successful lookup result.
  - `file`: `file::Handle` (The handle of the found object).
  - `file_attr`: `Option<file::Attr>` (Attributes of the found object).
  - `dir_attr`: `Option<file::Attr>` (Post-operation attributes of the parent directory).
- **Fail**: A structure representing a failed lookup result.
  - `error`: `vfs::Error` (The specific error encountered).
  - `dir_attr`: `Option<file::Attr>` (Post-operation attributes of the parent directory).
- **Lookup**: An asynchronous trait defining the lookup operation contract.

Relations:
- **Composition**: `Args` composes `file::Handle` and `file::Name`.
- **Composition**: `Success` and `Fail` compose `file::Attr` and `file::Handle`.
- **Dependency**: `Fail` depends on `vfs::Error`.

Global Invariants:
- **Args Invariants**: The `name` field is guaranteed to be valid (non-empty, no slashes) due to the `file::Name` type.
- **Result Invariants**: In a `Success` result, the `file` handle is guaranteed to be valid within the context of the file system. In a `Fail` result, the `error` field provides a valid reason for the failure.

## 5. Error Model

Error Types:
- `vfs::Error`

Error Propagation Strategy:
- Errors are returned explicitly within the `Fail` struct variant of the `Result` type returned by the `lookup` method. This allows the error to be paired with directory attributes (`dir_attr`), adhering to the NFSv3 requirement to provide cache consistency data even on failure.

Recoverability:
- Recoverable. The caller receives the `Fail` struct containing the error code and can decide how to handle it (e.g., propagate to the NFS client, retry, or log).

Panics:
- Allowed: No
- Conditions: The trait definition implies a standard asynchronous operation returning a `Result`. It does not prescribe panics for standard error conditions.

---

## 6. Traits

List which external traits this module implements:
- **std::marker::Send**: Implemented for the `Lookup` trait via the `#[trait_variant::make(Send)]` macro. This allows the trait object to be sent between threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the interface for resolving path components to file handles within the NFSv3 server implementation. The system contains a Virtual File System (VFS) layer that abstracts the details of various storage backends (e.g., local disk, in-memory) from the NFS protocol logic. A typical usage scenario of the system involves an NFS client requesting a file by path. The server processes this path component by component. For each component, it uses the `Lookup` trait defined in this module to find the corresponding file handle given the parent directory's handle and the current component name.

Inside the system, the following things happen and they use this module: The VFS implementation (which implements the `Lookup` trait) receives the `Args` struct containing a validated `file::Name` (ensuring no directory traversal attacks via `..` or slashes) and a `file::Handle`. It performs the directory lookup. If successful, it returns a `Success` struct containing the new `file::Handle` and attributes. This handle is then used in subsequent operations or returned to the client. If the lookup fails (e.g., file not found), it returns a `Fail` struct with a `vfs::Error`. Crucially, both `Success` and `Fail` can include `dir_attr` (post-operation attributes of the directory). This is used to implement Weak Cache Consistency (WCC), allowing the client to detect if the directory was modified by another client even if the specific lookup failed, which is vital for maintaining a consistent view of the file system in a distributed environment. Without this module, the VFS would lack a standardized way to perform the most fundamental operation in NFS—mapping names to handles—while supporting the specific attribute caching requirements of the protocol.