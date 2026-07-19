<!-- SPEC_HASH: b469951f56c2d476c2c606bad6612caf0592e08e3dabd956be8b44c4a3f4a02a -->
# Module Specification

Module: nfs_mamont::vfs::lookup
Rust File: src/vfs/lookup.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `Error` enum. This type is used in the `Fail` struct to standardize error reporting across the VFS layer, ensuring that lookup failures map to correct NFSv3 status codes.
- **`super::file`**: Used to import the `Handle`, `Name`, and `Attr` types. These types are used in `Args`, `Success`, and `Fail` to represent file system identifiers, validated entry names, and file metadata, respectively.
- **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` attribute on the `Lookup` trait. This macro generates a version of the trait that is safe to send across threads, which is required for the trait to be used as an object in an asynchronous, multi-threaded server context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `LOOKUP` procedure, which resolves a file name within a directory to a file handle.
- To enforce the contract that the lookup operation does not follow symbolic links, as per the NFSv3 specification.
- To structure the input and output data to match the requirements of the NFSv3 protocol, specifically handling the return of both the target file's attributes and the directory's post-operation attributes.

Inputs:
- **`Args`**: A structure containing:
  - `parent`: A `file::Handle` identifying the directory to search.
  - `name`: A `file::Name` representing the entry to look up.

Outputs:
- **`Result<Success, Fail>`**:
  - **`Success`**: Contains the `file::Handle` of the found object, optional `file::Attr` for the object, and optional `file::Attr` for the directory.
  - **`Fail`**: Contains a `vfs::Error` describing the failure and optional `file::Attr` for the directory.

Steps:
1. The consumer of the trait invokes the `lookup` method with a reference to `self` and the `Args` struct.
2. The implementation (defined elsewhere) searches the directory specified by `args.parent` for an entry matching `args.name`.
3. If the entry is found:
   - The implementation returns `Ok(Success)`.
   - The `Success` struct includes the handle for the file system object.
   - It may include the attributes of the object (`file_attr`) and the directory (`dir_attr`).
4. If the entry is not found or an error occurs:
   - The implementation returns `Err(Fail)`.
   - The `Fail` struct includes the specific `vfs::Error`.
   - It may include the attributes of the directory (`dir_attr`) to allow the client to update its cache despite the error.

Edge Cases:
- **Symbolic Links**: The interface explicitly documents that it does not follow symbolic links. If `args.name` refers to a symbolic link, the returned handle must refer to the link itself, not its target.
- **Attribute Availability**: The `file_attr` and `dir_attr` fields in `Success` and `Fail` are wrapped in `Option`. The implementation may choose not to fetch or return these attributes based on performance constraints or internal state.

Complexity:
- **Time**: O(1) for the interface definition. The actual complexity depends on the implementation of the `Lookup` trait (e.g., hash map lookup vs. disk scan).
- **Space**: O(1) for the data structures defined in this module.

Determinism:
- **Deterministic**: The interface definition itself is deterministic. The determinism of the `lookup` operation depends on the specific implementation of the trait.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
  - **`Handle`**: Used in `Args` to identify the parent directory and in `Success` to identify the resolved file system object. It acts as the opaque identifier required by the NFS protocol.
  - **`Name`**: Used in `Args` to specify the target entry. It guarantees that the name has already been validated (e.g., no path separators, length limits) before reaching the lookup logic.
  - **`Attr`**: Used in `Success` and `Fail` to return metadata. This supports the Weak Cache Consistency (WCC) mechanism of NFSv3 by allowing the server to return post-operation attributes of the directory and the file.

- **From `nfs_mamont::vfs`**:
  - **`Error`**: Used in `Fail` to categorize the failure. This allows the RPC layer to map the error to the correct NFS status code (e.g., `NoEntry` for "file not found", `Access` for permission denied).

---

## 4. Data Model

Entities:
- **`Args`**: Arguments for the lookup operation.
  - `parent`: `file::Handle`
  - `name`: `file::Name`
- **`Success`**: Successful result of the lookup.
  - `file`: `file::Handle`
  - `file_attr`: `Option<file::Attr>`
  - `dir_attr`: `Option<file::Attr>`
- **`Fail`**: Failed result of the lookup.
  - `error`: `vfs::Error`
  - `dir_attr`: `Option<file::Attr>`

Relations:
- **Composition**: `Args` composes `file::Handle` and `file::Name`.
- **Aggregation**: `Success` and `Fail` aggregate `file::Attr` (optionally) to provide metadata back to the client.
- **Association**: `Fail` is associated with `vfs::Error` to describe the nature of the failure.

Global Invariants:
- **No Link Traversal**: The `Lookup` trait guarantees that the operation does not follow symbolic links.
- **Attribute Optionality**: The presence of attributes in `Success` and `Fail` is not guaranteed by the interface; it is implementation-dependent.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type wrapped in the `Fail` struct. This enum covers standard NFSv3 errors (e.g., `NoEntry`, `IO`, `Access`) and implementation-specific errors.

Error Propagation Strategy:
- **Result Type**: The `lookup` method returns a `Result<Success, Fail>`. Errors are explicitly handled via the `Err` variant containing the `Fail` struct, which includes both the error code and potentially the directory attributes.

Recoverability:
- **Dependent on Error**: Recoverability is determined by the specific `vfs::Error` variant returned. For example, `JUKEBOX` suggests a retry is appropriate, while `NoEntry` indicates a permanent failure for that specific request.

Panics:
- **Allowed**: No. The interface defines a fallible return type (`Result`), so panics should not be used for standard error conditions.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`Lookup`**: An asynchronous trait (made `Send`) defining the `lookup` method for searching a directory.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **define the contract for resolving file paths within the NFSv3 Virtual File System (VFS)**. The system contains a complex architecture where the network handling layer (RPC) must interact with a generic storage backend without knowing the details of how files are stored. This module provides the specific interface for the `LOOKUP` procedure, which is fundamental to NFS operation as it translates a directory handle and a name into a new file handle.

A typical usage scenario of the system involves an NFS client requesting to open a file. The client sends a `LOOKUP` request containing a directory handle (obtained from a previous operation) and a filename. The RPC layer deserializes this request into the `Args` struct defined in this module. It then invokes the `lookup` method on the VFS implementation. The VFS implementation performs the search (e.g., in a local filesystem or a database) and returns a `Result<Success, Fail>`. The RPC layer then serializes the `Success` data (file handle and attributes) back to the client.

Inside the system, the following things happen and they use this module:
1.  **Protocol Enforcement**: The `vfs` module aggregates the `Lookup` trait into the main `Vfs` super-trait. This ensures that any storage backend plugged into the server supports the `LOOKUP` operation. The strict typing of `Args` (using `file::Handle` and `file::Name`) ensures that the backend receives validated, protocol-compliant inputs.
2.  **Cache Consistency**: The `Success` and `Fail` structs are designed to carry `dir_attr` (directory attributes). This is crucial for the NFSv3 Weak Cache Consistency model. Even if the lookup fails (e.g., permission denied), returning the directory's current attributes allows the client to update its cache of the directory's contents, preventing stale data.
3.  **Symbolic Link Handling**: The module explicitly documents that the lookup does not follow symbolic links. This is a critical semantic requirement for NFSv3, where `LOOKUP` is expected to return the handle of the link itself, leaving it to the client (or a subsequent `READLINK` operation) to resolve the target.

Without this module, the VFS layer would lack a standardized way to request path resolution, forcing the RPC layer to make assumptions about how the backend identifies files or how it reports lookup errors. This module isolates the semantics of "finding a file" into a single, well-defined interface.