<!-- SPEC_HASH: a7eefc86e24109e715d8c48d4a48af4981c8cdae808516eaa58a22d821348d29 -->
# Module Specification

Module: nfs_mamont::vfs::mk_dir
Rust File: src/vfs/mk_dir.rs

---

## 1. Dependencies

From the source code and context, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `WccData` struct and the `Error` enum. `WccData` is required in the return types (`Success` and `Fail`) to provide Weak Cache Consistency data for the parent directory. `Error` is used to define failure conditions, specifically `Exist` for invalid directory names.
- **`super::file`**: Used to import the `Handle` and `Attr` types. `Handle` is used in `Success` to identify the newly created directory. `Attr` is used in `Success` to return the attributes of the new directory.
- **`super::set_attr`**: Used to import the `NewAttr` struct. This is used in `Args` to allow the client to specify initial attributes (mode, uid, gid, etc.) for the directory at creation time.
- **`trait_variant::make`**: Used to transform the `MkDir` trait into a `Send` trait object. This allows the trait to be used in asynchronous contexts where the implementor must be safe to send across threads.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `MKDIR` procedure, allowing clients to create a new subdirectory within an existing directory.
- To enforce protocol-specific constraints, such as prohibiting the creation of "." or ".." entries.
- To return comprehensive post-operation data, including the new directory's handle, its attributes, and Weak Cache Consistency (WCC) data for the parent directory.

Inputs:
- **`args: Args`**: A structure containing:
 - `object`: A `vfs::DirOpArgs` identifying the parent directory handle and the name of the new subdirectory.
 - `attr`: A `set_attr::NewAttr` structure specifying the initial attributes for the new directory.

Outputs:
- **`Result<Success, Fail>`**:
 - `Success`: Contains `file` (Option<file::Handle>), `attr` (Option<file::Attr>), and `wcc_data` (vfs::WccData).
 - `Fail`: Contains `error` (vfs::Error) and `dir_wcc` (vfs::WccData).

Steps:
1. **Name Validation**: The implementation must check the `name` field within `Args::object`.
 - If the name is "." or "..", the implementation must return `Err(Fail)` with `vfs::Error::Exist`.
2. **Directory Creation**: The implementation must attempt to create a new directory with the specified name inside the directory identified by `Args::object.dir`.
3. **Attribute Initialization**: If `Args::attr` contains fields to be set (e.g., mode, uid, gid), the implementation must apply these attributes to the newly created directory.
4. **Data Collection**:
 - Retrieve the `file::Handle` and `file::Attr` for the newly created directory.
 - Capture the `vfs::WccData` for the parent directory (the directory specified in `Args::object`).
5. **Result Construction**:
 - On success, return `Ok(Success)` populated with the new handle, attributes, and WCC data.
 - On failure, return `Err(Fail)` populated with the error and the WCC data for the parent directory.

Edge Cases:
- **Reserved Names**: The interface explicitly requires returning `vfs::Error::Exist` if the client attempts to create a directory named "." or "..".
- **Optional Return Fields**: The `Success` struct defines `file` and `attr` as `Option`. While the NFSv3 protocol typically requires these, the Rust interface allows for `None` values, potentially for implementations that cannot retrieve them immediately or in specific error scenarios not detailed here.

Complexity:
- **Time**: Dependent on the implementation (backend I/O), but the interface definition implies O(1) logic for argument validation.
- **Space**: O(1) for the argument and result structures.

Determinism:
- **Non-deterministic**: The result depends on the state of the file system (e.g., whether the directory already exists) and the success of I/O operations.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: This module relies on the `WccData` structure to fulfill the NFSv3 requirement of returning pre- and post-operation attributes for the parent directory. This allows clients to validate their caches for the parent directory without re-reading it.
 - **`Error`**: The module uses the `vfs::Error` enum to standardize error reporting. Specifically, it utilizes `Exist` to signal protocol violations regarding reserved names ("." or "..").

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: Used to uniquely identify the newly created directory in the `Success` result.
 - **`Attr`**: Used to return the full metadata of the newly created directory in the `Success` result.

- **From `nfs_mamont::vfs::set_attr`**:
 - **`NewAttr`**: This module uses `NewAttr` to allow the client to pass initial attributes (like permissions or ownership) during the creation process. This integrates the creation logic with the attribute setting logic defined in the `set_attr` module.

---

## 4. Data Model

Entities:
- **`Args`**: The input arguments for the `mk_dir` operation.
 - Fields: `object` (vfs::DirOpArgs), `attr` (set_attr::NewAttr).
- **`Success`**: The successful result wrapper.
 - Fields: `file` (Option<file::Handle>), `attr` (Option<file::Attr>), `wcc_data` (vfs::WccData).
- **`Fail`**: The failure result wrapper.
 - Fields: `error` (vfs::Error), `dir_wcc` (vfs::WccData).

Relations:
- **Composition**: `Args` aggregates `DirOpArgs` (which contains `Handle` and `Name`) and `NewAttr`.
- **Association**: `Success` and `Fail` both contain `vfs::WccData`, linking the operation result to the state of the parent directory.

Global Invariants:
- **Reserved Name Check**: If the name in `Args::object` is "." or "..", the operation must fail with `vfs::Error::Exist`.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type returned within the `Fail` struct.

Error Propagation Strategy:
- **Direct Return**: Errors are returned wrapped in the `Fail` struct. The `Fail` struct also includes `dir_wcc` to reflect the state of the parent directory after the failed attempt.

Recoverability:
- **`vfs::Error::Exist`**: Indicates that the directory already exists or the client tried to create a reserved name ("." or ".."). The client should not retry the same request immediately.
- **Other Errors**: Standard I/O or permission errors where recovery depends on the specific error code.

Panics:
- **Allowed**: No. The interface defines a contract for returning errors via `Result`, not for panicking.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`MkDir`**: An asynchronous trait defining the `mk_dir` method. It is marked `Send` via `trait_variant`, allowing it to be used as a trait object in multi-threaded async contexts.

---

## 7. Overview

This module is used in order to **define the contract for creating directories** within the `nfs_mamont` NFSv3 server. The system requires a specialized interface for this operation because the NFSv3 `MKDIR` procedure has specific semantics that differ from a generic "create directory" command. Specifically, it requires the return of the new directory's handle and attributes, along with Weak Cache Consistency (WCC) data for the parent directory, and enforces specific error handling for reserved directory names.

A typical usage scenario of the system involves a client wishing to create a new directory named "data" inside an existing directory. The RPC layer receives the request, deserializes the arguments into the `Args` struct (which includes the parent directory handle, the name "data", and optional initial attributes), and invokes the `mk_dir` method on the VFS backend. The backend validates the name (ensuring it is not "." or ".."), creates the directory, applies the initial attributes, and returns the `Success` struct. This struct contains the `Handle` for the new directory (which the client uses for future operations) and the `WccData` for the parent directory (which the client uses to update its cache).

Inside the system, the following things happen and they use this module:
1.  **Protocol Enforcement**: The module explicitly handles the NFSv3 requirement that creating "." or ".." must fail with `Error::Exist`. This prevents the client from corrupting the directory structure by overwriting reserved entries.
2.  **Cache Coherency**: By requiring `WccData` in both `Success` and `Fail` results, this module enforces the NFSv3 requirement that the client must be informed about changes to the parent directory's attributes (such as the modification time or link count) that result from the creation operation.
3.  **Atomic Initialization**: The use of `set_attr::NewAttr` in the arguments allows the client to atomically set the directory's permissions and ownership during creation, rather than requiring a separate `SETATTR` call after creation.

Without this module, the VFS layer would lack a standardized way to handle the nuanced requirements of directory creation in NFSv3, leading to potential inconsistencies in cache handling and non-compliance with the protocol's error codes for reserved names.