<!-- SPEC_HASH: 282e1c29d4eab474bedae8331053d0b469f5f9fc941e986f4497f8ee1193db2d -->
# Module Specification

Module: nfs_mamont::vfs::create
Rust File: src/vfs/create.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::consts::nfsv3`**: Used to import the constant `NFS3_CREATEVERFSIZE`. This constant defines the fixed size of the `Verifier` byte array, ensuring compliance with the NFSv3 protocol specification for exclusive file creation.
- **`crate::vfs`**: Used to import the `WccData` struct and the `Error` enum. `WccData` is required in the `Success` and `Fail` structs to provide Weak Cache Consistency data for the parent directory. `Error` is used to define failure conditions, such as `vfs::Error::Exist` when a duplicate file is detected in `Guarded` mode.
- **`super::file`**: Used to import the `Handle` and `Attr` types. These are used within the `Success` struct to return the identity and metadata of the newly created file to the client.
- **`super::set_attr`**: Used to import the `NewAttr` struct. This is used within the `How` enum (`Unchecked` and `Guarded` variants) to specify the initial attributes (mode, UID, GID, size, timestamps) for the file being created.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `CREATE` procedure, which creates a regular file.
- To support three distinct creation semantics defined by the NFSv3 protocol: `Unchecked` (create regardless), `Guarded` (create only if not exists), and `Exclusive` (create atomically using a verifier).
- To return the necessary metadata (file handle, attributes) and cache consistency data (WCC data) for the parent directory to the client.

Inputs:
- **`args: Args`**: A structure containing:
 - `object`: `vfs::DirOpArgs` specifying the directory handle and the name of the file to create.
 - `how`: `How` enum specifying the creation mode and associated data (attributes or verifier).

Outputs:
- **`Result<Success, Fail>`**:
 - `Success`: Contains `file` (Option<file::Handle>), `attr` (Option<file::Attr>), and `wcc_data` (vfs::WccData).
 - `Fail`: Contains `error` (vfs::Error) and `wcc_data` (vfs::WccData).

Steps:
1. **Mode Inspection**: The implementation inspects the `args.how` variant to determine the creation strategy.
2. **Unchecked Execution**:
 - If `How::Unchecked`, the implementation creates the file with the attributes specified in the `NewAttr` payload.
 - No check for pre-existence is performed.
3. **Guarded Execution**:
 - If `How::Guarded`, the implementation checks if the file already exists in the target directory.
 - If it exists, the operation fails with `vfs::Error::Exist`.
 - If it does not exist, the file is created as in `Unchecked` mode.
4. **Exclusive Execution**:
 - If `How::Exclusive`, the implementation uses the provided `Verifier` to ensure exclusive creation.
 - The server must verify that the file does not exist or that it can be created atomically such that the verifier guarantees uniqueness. The implementation may store the verifier in the file's metadata (e.g., `ctime`).
 - No attributes are provided in this mode.
5. **Response Construction**:
 - On success, the implementation returns `Success` containing the new file handle, the file's attributes, and the `WccData` for the directory (reflecting the change in link count, etc.).
 - On failure, the implementation returns `Fail` containing the specific `vfs::Error` and the `WccData` for the directory (which may reflect pre-operation attributes if the directory was not modified).

Edge Cases:
- **Exclusive Verifier Storage**: The mechanism for storing the `Verifier` is implementation-dependent. The code comment suggests the server "may use the target file metadata to store the Verifier," implying the backend has flexibility in how it validates the exclusive create.
- **Optional Return Fields**: The `Success` struct defines `file` and `attr` as `Option`. While the NFSv3 protocol typically mandates these on success, the Rust interface allows for `None`, potentially to handle edge cases or specific backend limitations where the handle/attr cannot be immediately retrieved.

Complexity:
- Time: Dependent on the backend implementation (file system I/O).
- Space: O(1) for the argument and result structures (excluding backend allocation).

Determinism:
- Non-deterministic. The result depends on the current state of the file system (e.g., whether the file exists in `Guarded` mode).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: This module relies on `WccData` to fulfill the NFSv3 requirement of returning pre- and post-operation attributes for the directory. This allows clients to validate their directory caches without re-reading the directory contents.
 - **`Error`**: The module uses `vfs::Error` to standardize error reporting. Specifically, it utilizes `vfs::Error::Exist` to signal a failure in `Guarded` mode due to a duplicate file name.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle` and `Attr`**: These types are used in the `Success` struct to return the identity and metadata of the created file. The `Handle` serves as the unique reference for future client operations, and `Attr` provides the initial state.

- **From `nfs_mamont::vfs::set_attr`**:
 - **`NewAttr`**: This struct is used in `How::Unchecked` and `How::Guarded` to pass the initial attributes for the file. This allows the client to set the mode, ownership, size, and timestamps at the moment of creation, rather than requiring a separate `SETATTR` call.

---

## 4. Data Model

Entities:
- **`Verifier`**: A newtype wrapper around a fixed-size byte array `[u8; NFS3_CREATEVERFSIZE]`. It acts as an opaque token for exclusive creation.
- **`How`**: An enum defining the creation strategy.
 - `Unchecked(super::set_attr::NewAttr)`: Create with attributes, ignore existence.
 - `Guarded(super::set_attr::NewAttr)`: Create with attributes, fail if exists.
 - `Exclusive(Verifier)`: Create exclusively using the verifier.
- **`HowMode`**: An enum with integer discriminants (`Unchecked = 0`, `Guarded = 1`, `Exclusive = 2`) mapping `How` variants to protocol codes.
- **`Args`**: Arguments for the `create` operation.
 - `object`: `vfs::DirOpArgs` (directory handle and filename).
 - `how`: `How` (creation mode).
- **`Success`**: Result on success.
 - `file`: `Option<file::Handle>`.
 - `attr`: `Option<file::Attr>`.
 - `wcc_data`: `vfs::WccData`.
- **`Fail`**: Result on failure.
 - `error`: `vfs::Error`.
 - `wcc_data`: `vfs::WccData`.

Relations:
- **Composition**: `Args` aggregates `vfs::DirOpArgs` and `How`.
- **Association**: `How` is associated with `set_attr::NewAttr` or `Verifier`.
- **Association**: `Success` and `Fail` are associated with `vfs::WccData`.

Global Invariants:
- **Verifier Size**: The `Verifier` array size is strictly defined by `NFS3_CREATEVERFSIZE`.
- **Mode Mapping**: The discriminants of `HowMode` must correspond to the integer values used in the NFSv3 protocol for the `how` field of the `CREATE` arguments.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type returned within the `Fail` struct.

Error Propagation Strategy:
- **Direct Return**: Errors are returned wrapped in the `Fail` struct. The `Fail` struct also includes `wcc_data` to reflect the state of the directory (pre-operation attributes if the directory was not modified).

Recoverability:
- **`vfs::Error::Exist`**: In `Guarded` mode, this is an expected outcome indicating the file already exists. The client typically handles this by not proceeding or by taking alternative action.
- **Other Errors**: Standard I/O or permission errors where recovery depends on the specific error code.

Panics:
- **Allowed**: No. The interface defines a contract for returning errors via `Result`, not for panicking.

---

## 6. Traits

List which external traits this module implements:
- **`Send`**: The `Create` trait is transformed into a `Send` trait object using `#[trait_variant::make(Send)]`, allowing it to be used in asynchronous contexts where the implementor must be safe to send across threads.

List which traits this module defines:
- **`Create`**: An asynchronous trait defining the `create` method for creating regular files.

---

## 7. Overview

This module is used in order to **define the contract for creating regular files** within the `nfs_mamont` NFSv3 server. The system requires a specialized interface for this operation because the NFSv3 `CREATE` procedure encompasses more complex logic than a simple "create file" system call. It must handle three distinct modes of operation (`Unchecked`, `Guarded`, `Exclusive`) that dictate how the server handles pre-existing files and how initial attributes are applied.

A typical usage scenario of the system involves a client requesting the creation of a new file. The RPC layer receives the request, deserializes the arguments into the `Args` struct (which includes the directory location and the `How` mode), and invokes the `create` method on the VFS backend. If the mode is `Guarded`, the backend checks for existence and returns `vfs::Error::Exist` if the file is already there. If the mode is `Exclusive`, the backend uses the `Verifier` to ensure that the creation is atomic and unique, preventing race conditions. Regardless of the mode, the backend returns `WccData` for the parent directory, allowing the client to update its cache efficiently (e.g., noticing the increased link count) without performing a separate directory read.

Inside the system, the following things happen and they use this module:
1. **Protocol Semantics Enforcement**: The `How` enum enforces the distinction between the three NFSv3 creation methods. This ensures that the backend implementation explicitly handles the "check for existence" logic required by `Guarded` mode and the "verifier" logic required by `Exclusive` mode, rather than relying on a generic `O_CREAT | O_EXCL` flag which might not map perfectly to the protocol's requirements (especially regarding attribute initialization).
2. **Attribute Initialization**: By including `set_attr::NewAttr` in the `Unchecked` and `Guarded` variants, this module allows the client to set the file's mode, UID, GID, size, and timestamps atomically during creation. This is more efficient and atomic than creating the file and then calling `SETATTR` immediately after.
3. **Cache Consistency**: By requiring `WccData` in both `Success` and `Fail` results, this module ensures that the client is always informed about the state of the parent directory, maintaining the Weak Cache Consistency model essential for NFS performance.

Without this module, the VFS layer would lack a standardized way to handle the nuanced requirements of file creation in NFSv3, leading to potential race conditions, incorrect attribute handling, and inefficient cache management on the client side.