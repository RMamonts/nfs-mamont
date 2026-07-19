<!-- SPEC_HASH: a7eefc86e24109e715d8c48d4a48af4981c8cdae808516eaa58a22d821348d29 -->
# Module Specification

Module: nfs_mamont::vfs::mk_dir
Rust File: src/vfs/mk_dir.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::vfs**: Used to import `vfs::DirOpArgs`, `vfs::WccData`, and `vfs::Error`. `vfs::DirOpArgs` is used in `Args` to specify the target directory and the new name. `vfs::WccData` is used in `Success` and `Fail` to provide Weak Cache Consistency data for the parent directory. `vfs::Error` is used in `Fail` to report the specific reason for the operation failure.
- **super::file**: Used to import `file::Handle` and `file::Attr`. `file::Handle` is used in `Success` to return the identifier of the newly created directory. `file::Attr` is used in `Success` to return the attributes of the newly created directory.
- **super::set_attr**: Used to import `set_attr::NewAttr`. This type is used in `Args` to specify the initial attributes (mode, uid, gid, etc.) for the new directory.
- **trait_variant::make**: Used to transform the `MkDir` trait into a `Send` trait object. This is necessary to allow the implementation of the VFS to be passed across thread boundaries in an asynchronous context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for creating a new directory within the Virtual File System (VFS) layer, corresponding to the NFSv3 `MKDIR` procedure.
- To enforce specific validation rules regarding directory names (specifically rejecting "." and "..").
- To provide a mechanism for setting initial attributes on the newly created directory atomically (or near-atomically) with its creation.
- To return Weak Cache Consistency (WCC) data for the parent directory to allow clients to validate their cache.

Inputs:
- `Args`: A structure containing:
 - `object`: A `vfs::DirOpArgs` identifying the parent directory (via `file::Handle`) and the name of the new directory (via `file::Name`).
 - `attr`: A `set_attr::NewAttr` structure specifying the initial attributes for the new directory.

Outputs:
- `Result<Success, Fail>`:
 - `Success`: Contains the `file::Handle` and `file::Attr` of the created directory, along with `vfs::WccData` for the parent directory.
 - `Fail`: Contains a `vfs::Error` describing the failure and `vfs::WccData` for the parent directory.

Steps:
1. **Name Validation**: The implementation must inspect the `name` field within `Args::object`.
 - If the name is "." or "..", the operation must fail immediately, returning `Err(Fail)` where `error` is `vfs::Error::Exist`.
2. **Directory Creation**: The implementation attempts to create a new subdirectory with the specified name inside the directory identified by `Args::object.dir`.
3. **Attribute Initialization**: The implementation applies the attributes specified in `Args::attr` to the newly created directory. This involves interpreting the `set_attr::NewAttr` structure (e.g., setting mode, uid, gid).
4. **Handle Generation**: The implementation generates a `file::Handle` that uniquely identifies the new directory.
5. **WCC Data Collection**: The implementation captures the state of the parent directory (identified by `Args::object.dir`) before and after the operation to construct `vfs::WccData`.
6. **Result Construction**:
 - On success: Returns `Ok(Success)` with the new handle, attributes, and parent WCC data.
 - On failure: Returns `Err(Fail)` with the specific error and parent WCC data.

Edge Cases:
- **Reserved Names**: The operation explicitly treats "." and ".." as existing entries, returning `vfs::Error::Exist` rather than creating a directory with these names.
- **Attribute Application**: If the underlying file system does not support setting specific attributes during creation, the behavior depends on the implementation, but the interface implies the intent to set them.

Complexity:
- Time: Dependent on the underlying file system implementation. The interface itself involves simple data structure access.
- Space: O(1) for the arguments and return structures.

Determinism:
- Deterministic (assuming the underlying file system state is stable).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs` (Assumption based on facts)**: `vfs::DirOpArgs` is a struct containing `dir: file::Handle` and `name: file::Name`. This mechanism is critical for identifying the target of the creation operation. `vfs::WccData` is a struct containing `before: Option<file::WccAttr>` and `after: Option<file::Attr>`. This mechanism is critical for the `Success` and `Fail` return types to allow the client to verify its cache state regarding the parent directory. `vfs::Error` is an enum defining specific error codes like `Exist` (value 17), which is returned for "." or ".." names.
- **From `crate::vfs::set_attr`**: `set_attr::NewAttr` is a struct collecting requested attribute changes (mode, uid, gid, size, atime, mtime). In this module, it is used to define the initial state of the directory, rather than modifying an existing one.
- **From `crate::vfs::file`**: `file::Handle` is a fixed-size byte array `[u8; NFS3_FHSIZE]` acting as an opaque identifier for the file. `file::Attr` is a structure containing comprehensive file metadata.

---

## 4. Data Model

Entities:
- **Args**: The input arguments for the operation.
 - `pub object: vfs::DirOpArgs`
 - `pub attr: super::set_attr::NewAttr`
- **Success**: The successful result wrapper.
 - `pub file: Option<file::Handle>` (The handle for the new directory).
 - `pub attr: Option<file::Attr>` (The attributes of the new directory).
 - `pub wcc_data: vfs::WccData` (WCC data for the parent directory).
- **Fail**: The failure result wrapper.
 - `pub error: vfs::Error`
 - `pub dir_wcc: vfs::WccData` (WCC data for the parent directory).

Relations:
- **Composition**: `Args` aggregates `vfs::DirOpArgs` and `set_attr::NewAttr`.
- **Association**: `Success` and `Fail` both contain `vfs::WccData`.

Global Invariants:
- **Name Invariant**: The implementation must reject "." and ".." as directory names, returning `vfs::Error::Exist`.

## 5. Error Model

Error Types:
- `vfs::Error`

Error Propagation Strategy:
- The method returns a `Result<Success, Fail>`. The `Fail` struct wraps the `vfs::Error` along with `vfs::WccData`.

Recoverability:
- Recoverable. The client receives the error code and the `WccData` (which contains the pre-operation attributes in `before`), allowing it to synchronize its cache.

Panics:
- Allowed: No
- Conditions: The trait definition does not specify panics. Implementations should return `vfs::Error` for expected failure modes.

---

## 6. Traits

List which external traits this module implements:
- **std::marker::Send**: Implemented for `MkDir` via the `#[trait_variant::make(Send)]` macro. This allows the trait object to be sent between threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the interface for creating directories within the `nfs_mamont` NFSv3 server implementation. The system contains a Virtual File System (VFS) layer that abstracts the underlying storage from the NFS protocol logic. A typical usage scenario involves an NFS client sending an `MKDIR` request to create a new folder. The server decodes this request into the `Args` struct defined in this module and invokes the `mk_dir` method on the VFS implementation.

Inside the system, the following things happen and they use this module: The VFS implementation uses the `Args` struct to determine the target parent directory and the name of the new directory. It specifically checks for the reserved names "." and ".." to prevent logical errors in the file system hierarchy, returning `vfs::Error::Exist` if they are encountered. The `attr` field of `Args`, which relies on the `set_attr::NewAttr` type from the dependency, allows the client to specify permissions (mode), ownership (uid/gid), and timestamps for the new directory immediately upon creation, rather than requiring a separate `SETATTR` call. The result of the operation, whether success or failure, includes `vfs::WccData` for the parent directory. This is crucial for the NFS protocol's Weak Cache Consistency model, as it allows the client to update its cache of the parent directory's contents (e.g., the link count) without performing a separate lookup. Without this module, the VFS would lack a standardized way to handle directory creation requests, including the necessary validation and cache consistency data required by the NFSv3 protocol.