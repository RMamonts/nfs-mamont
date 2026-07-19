<!-- SPEC_HASH: afb2f79fed336ad900abf0ac87edf4f7fcd2550ca735ad5c67e68993b871d842 -->
# Module Specification

Module: nfs_mamont::vfs::symlink
Rust File: src/vfs/symlink.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `WccData` struct, the `Error` enum, and the `DirOpArgs` struct. `WccData` is required in the return types to provide Weak Cache Consistency data for the directory being modified. `Error` is used to define failure conditions, specifically `Exist` for invalid names. `DirOpArgs` is used in `Args` to specify the target directory and the new link's name.
- **`super::file`**: Used to import the `Handle`, `Attr`, and `Path` types. `Handle` and `Attr` are returned in `Success` to identify and describe the newly created link. `Path` is used in `Args` to store the target path of the symbolic link.
- **`super::set_attr`**: Used to import the `NewAttr` struct. This is used in `Args` to allow the client to specify initial attributes (mode, uid, gid, etc.) for the symbolic link at the moment of creation.
- **`trait_variant::make`**: Used to transform the `Symlink` trait into a `Send` trait object. This allows the trait to be used in asynchronous contexts where the implementor must be safe to send across threads.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `SYMLINK` procedure, enabling the creation of symbolic links.
- To enforce strict atomicity requirements where the creation of the file system node and the population of its content (the target path) must appear as a single indivisible operation.
- To enforce specific naming constraints, specifically rejecting "." and ".." as valid names for symbolic links.

Inputs:
- **`args: Args`**: A structure containing:
 - `object`: A `vfs::DirOpArgs` specifying the directory handle where the link will be created and the name of the link.
 - `attr`: A `set_attr::NewAttr` structure specifying the initial attributes for the new link.
 - `path`: A `file::Path` representing the target path to which the symbolic link will point.

Outputs:
- **`Result<Success, Fail>`**:
 - `Success`: Contains `file` (Option<file::Handle>), `attr` (Option<file::Attr>), and `wcc_data` (vfs::WccData) representing the state of the directory after the operation.
 - `Fail`: Contains `error` (vfs::Error) indicating the failure reason and `dir_wcc` (vfs::WccData) representing the state of the directory.

Steps:
1. **Name Validation**: The implementation must check the `name` field within `Args::object`.
 - If the name is "." or "..", the implementation must return `Err(Fail)` with `vfs::Error::Exist`.
2. **Atomic Creation**: The implementation must create the symbolic link node within the directory specified by `Args::object.dir`.
 - The content of the link (the data stored in `Args::path`) must be written atomically with the creation of the node.
 - **Constraint**: There must be no window where the link is visible in the directory but a `read_link` operation would fail or return incorrect data.
3. **Attribute Application**: The implementation must apply the attributes specified in `Args::attr` to the newly created link.
4. **WCC Data Collection**: The implementation must capture the attributes of the directory before the operation (if possible) and after the operation to populate `vfs::WccData`.
5. **Result Construction**:
 - On success, return `Ok(Success)` with the new `Handle` and `Attr`.
 - On failure, return `Err(Fail)` with the specific `vfs::Error` and the directory's `WccData`.

Edge Cases:
- **Reserved Names**: The names "." and ".." are explicitly forbidden and must result in `vfs::Error::Exist`, rather than `IO` or `Access` errors.
- **Atomicity Violation**: If the underlying file system cannot create the link and its content atomically, it violates the contract of this trait.

Complexity:
- Time: Dependent on the implementation (backend I/O), but the interface definition implies O(1) logic for argument validation.
- Space: O(1) for the argument and result structures.

Determinism:
- Non-deterministic. The result depends on the state of the file system and the success of I/O operations.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: This module relies on `WccData` to fulfill the NFSv3 requirement of returning pre- and post-operation attributes for the directory containing the new link.
 - **`DirOpArgs`**: This module uses `DirOpArgs` to encapsulate the target directory handle and the new entry name, standardizing how directory modification operations are invoked.
 - **`Error`**: The module uses the `vfs::Error` enum to standardize error reporting. Specifically, it utilizes `Exist` to signal the use of reserved names ("." or "..").

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: Used to uniquely identify the newly created symbolic link in the `Success` result.
 - **`Attr`**: Used to return the full metadata of the newly created link in the `Success` result.
 - **`Path`**: Used to represent the content of the symbolic link (the target path) in the `Args`.

- **From `nfs_mamont::vfs::set_attr`**:
 - **`NewAttr`**: Used to allow the client to specify initial attributes (mode, uid, gid, size, timestamps) for the symbolic link at creation time, avoiding a separate `SETATTR` call.

---

## 4. Data Model

Entities:
- **`Success`**: The successful result wrapper.
 - Fields: `file` (Option<file::Handle>), `attr` (Option<file::Attr>), `wcc_data` (vfs::WccData).
- **`Fail`**: The failure result wrapper.
 - Fields: `error` (vfs::Error), `dir_wcc` (vfs::WccData).
- **`Args`**: The input arguments for the `symlink` operation.
 - Fields: `object` (vfs::DirOpArgs), `attr` (set_attr::NewAttr), `path` (file::Path).

Relations:
- **Composition**: `Args` aggregates `DirOpArgs`, `NewAttr`, and `Path`.
- **Association**: `Success` and `Fail` both contain `vfs::WccData` (named `wcc_data` and `dir_wcc` respectively) to report directory state changes.

Global Invariants:
- **Atomic Visibility**: Once the `symlink` operation returns success, the symbolic link must be fully formed and readable via `read_link`.
- **Name Restrictions**: The names "." and ".." are permanently invalid for symbolic links in this interface and must always trigger `vfs::Error::Exist`.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type returned within the `Fail` struct.

Error Propagation Strategy:
- **Direct Return**: Errors are returned wrapped in the `Fail` struct. The `Fail` struct also includes `dir_wcc` to reflect the state of the directory after the failed attempt.

Recoverability:
- **`vfs::Error::Exist`**: Indicates the client attempted to use a reserved name ("." or ".."). The client must choose a different name.
- **Other Errors**: Standard I/O or permission errors where recovery depends on the specific error code.

Panics:
- **Allowed**: No. The interface defines a contract for returning errors via `Result`, not for panicking.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`Symlink`**: An asynchronous trait defining the `symlink` method. It is marked `Send` via `trait_variant`, allowing it to be used as a trait object in multi-threaded async contexts.

---

## 7. Overview

This module is used in order to **define the contract for creating symbolic links** within the `nfs_mamont` NFSv3 server. The system requires a specialized interface for this operation because the NFSv3 `SYMLINK` procedure has specific semantics regarding atomicity and initial attribute setting that differ from a generic "create file" operation. Specifically, it mandates that the link node and its content (the target path) must be created atomically, ensuring that no race condition exists where the link appears but is unreadable.

A typical usage scenario of the system involves a client requesting the creation of a symbolic link from path A to path B. The RPC layer receives this request, deserializes the arguments into the `Args` struct (containing the parent directory, the new link name, the target path, and initial attributes), and invokes the `symlink` method on the VFS backend. The backend implementation validates the name (rejecting "." and ".."), creates the link atomically, and returns the `Success` struct containing the new `Handle` and `Attr`, along with `WccData` for the parent directory.

Inside the system, the following things happen and they use this module:
1. **Atomicity Enforcement**: The trait documentation explicitly requires that the file system node and its contents be created in a single atomic operation. This is critical for distributed file systems to prevent clients from attempting to read a link that is in an intermediate, incomplete state.
2. **Initial Attribute Setting**: By including `set_attr::NewAttr` in the `Args`, this module allows the client to set the mode, ownership, and other metadata of the link at the moment of creation. This is more efficient and atomic than creating the link with default attributes and then immediately calling `SETATTR`.
3. **Protocol-Specific Validation**: The requirement to return `vfs::Error::Exist` for "." and ".." aligns with NFSv3 protocol behaviors where these specific names are treated as errors in this context, rather than generic invalid input errors.

Without this module, the VFS layer would lack a standardized way to handle the creation of symbolic links with the required atomicity guarantees and attribute initialization, potentially leading to race conditions or non-compliant behavior regarding reserved names.