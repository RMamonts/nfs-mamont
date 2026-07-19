<!-- SPEC_HASH: af02f1740d9f30daec120ec3e35320104c1bc76b3951645826e7f2c5d730bde4 -->
# Module Specification

Module: nfs_mamont::vfs::mk_node
Rust File: src/vfs/mk_node.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `WccData` struct, `DirOpArgs` struct, and the `Error` enum. `WccData` is required in the return types to provide Weak Cache Consistency data for the parent directory. `DirOpArgs` is used to specify the target directory and name for the new node. `Error` is used to define specific failure conditions such as `NotSupported` or `BadType`.
- **`super::file`**: Used to import the `Handle`, `Attr`, and `Device` types. `Handle` and `Attr` are used in the `Success` struct to identify and describe the newly created file. `Device` is used within the `What` enum to provide major and minor numbers for character and block device creation.
- **`super::set_attr::NewAttr`**: Used within the `What` enum to specify the initial attributes (mode, uid, gid, size, timestamps) for the special file being created.
- **`trait_variant::make`**: Used to transform the `MkNode` trait into a `Send` trait object. This allows the trait to be used in asynchronous contexts where the implementor must be safe to send across threads.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `MKNODE` procedure, which is responsible for creating special file system objects (character devices, block devices, sockets, and FIFOs).
- To encapsulate the specific arguments required for different types of special files (e.g., device numbers for devices vs. just attributes for sockets) using a discriminated union (`What`).
- To enforce the return of Weak Cache Consistency (WCC) data for the parent directory, ensuring clients can validate their directory caches after the creation operation.

Inputs:
- **`args: Args`**: A structure containing:
 - `object`: A `vfs::DirOpArgs` identifying the parent directory and the name of the new node.
 - `what`: A `What` enum variant specifying the type of node to create (`Char`, `Block`, `Socket`, `Fifo`), along with its initial attributes and optional device numbers.

Outputs:
- **`Result<Success, Fail>`**:
 - `Success`: Contains `file` (Option<file::Handle>), `attr` (Option<file::Attr>), and `wcc_data` (vfs::WccData).
 - `Fail`: Contains `error` (vfs::Error) and `dir_wcc` (vfs::WccData).

Steps:
1. **Type Validation**: The implementation inspects the `what` field of `Args`.
 - If the server implementation does not support the `MKNODE` procedure at all, it must return `vfs::Error::NotSupported`.
 - If the server supports `MKNODE` but does not support the specific type requested (e.g., trying to create a socket on a filesystem that doesn't support them), it must return `vfs::Error::BadType`.
2. **Node Creation**: The implementation attempts to create the file system entry within the directory specified by `Args::object`.
 - For `Char` and `Block` variants, it uses the provided `file::Device` (major/minor numbers) and `NewAttr`.
 - For `Socket` and `Fifo` variants, it uses the provided `NewAttr`.
3. **Attribute Assignment**: The attributes specified in `NewAttr` (mode, uid, gid, etc.) are applied to the new node.
4. **WCC Data Collection**: The implementation must capture the attributes of the parent directory before the operation (if possible) and after the operation to populate `vfs::WccData`.
5. **Result Construction**:
 - On success, return `Ok(Success)` containing the handle and attributes of the new node, along with the directory's `WccData`.
 - On failure, return `Err(Fail)` containing the specific error and the directory's `WccData` (reflecting the state of the directory, which likely did not change, but must be reported).

Edge Cases:
- **Optional Success Fields**: The `Success` struct defines `file` and `attr` as `Option`. While the NFSv3 protocol typically requires these fields on success, the Rust interface allows them to be optional. *Uncertainty: The specific conditions under which these would be `None` on success are not defined in the code, implying they should generally be `Some`.*
- **Unsupported Types**: The distinction between `NotSupported` (operation not available) and `BadType` (specific node type not available) must be strictly maintained.

Complexity:
- **Time**: Dependent on the implementation (backend I/O), but the interface definition implies O(1) logic for argument validation and dispatch.
- **Space**: O(1) for the argument and result structures.

Determinism:
- **Non-deterministic**: The result depends on the state of the file system and the success of I/O operations.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
 - **`WccData`**: This module relies on `WccData` to fulfill the NFSv3 requirement of returning pre- and post-operation attributes for the parent directory. This allows clients to validate their directory caches without re-reading the directory.
 - **`DirOpArgs`**: Used to locate the target directory and specify the name of the new node, providing the necessary context for the creation operation.
 - **`Error`**: The module uses specific variants of the `vfs::Error` enum to signal capability issues: `NotSupported` if the server cannot perform the operation, and `BadType` if the specific file type is invalid for the filesystem.

- **From `nfs_mamont::vfs::file`**:
 - **`Device`**: Used in the `Char` and `Block` variants of the `What` enum to carry the major and minor numbers required to identify device files.
 - **`Handle` and `Attr`**: Used in the `Success` struct to return the identity and metadata of the newly created special file to the client.

- **From `nfs_mamont::vfs::set_attr`**:
 - **`NewAttr`**: Used within the `What` enum to pass the initial attributes (mode, ownership, size) for the new file. This reuses the attribute setting logic defined in the `set_attr` module to ensure consistency in how attributes are applied during creation.

---

## 4. Data Model

Entities:
- **`What`**: A discriminated union (enum) identifying the type of special file to create.
 - Variants: `Char(NewAttr, file::Device)`, `Block(NewAttr, file::Device)`, `Socket(NewAttr)`, `Fifo(NewAttr)`.
- **`Args`**: Arguments for the `mk_node` operation.
 - Fields: `object` (vfs::DirOpArgs), `what` (What).
- **`Success`**: Wrapper for a successful operation result.
 - Fields: `file` (Option<file::Handle>), `attr` (Option<file::Attr>), `wcc_data` (vfs::WccData).
- **`Fail`**: Wrapper for a failed operation result.
 - Fields: `error` (vfs::Error), `dir_wcc` (vfs::WccData).

Relations:
- **Composition**: `Args` aggregates `What`.
- **Association**: `Success` and `Fail` both contain `vfs::WccData` (named `wcc_data` and `dir_wcc` respectively).

Global Invariants:
- **Type Specificity**: The `What` enum strictly enforces that `Device` numbers are provided only for `Char` and `Block` types, while `Socket` and `Fifo` types only require attributes.
- **Error Semantics**: The implementation must distinguish between the inability to perform the operation (`NotSupported`) and the inability to create the specific type (`BadType`).

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type returned within the `Fail` struct. Specific relevant variants include `NotSupported` and `BadType`.

Error Propagation Strategy:
- **Direct Return**: Errors are returned wrapped in the `Fail` struct. The `Fail` struct also includes `dir_wcc` to reflect the state of the parent directory.

Recoverability:
- **`vfs::Error::NotSupported`**: Indicates the server does not implement the `MKNODE` procedure. The client should not retry this operation on this server.
- **`vfs::Error::BadType`**: Indicates the requested file type is invalid for the target filesystem. The client should not retry creating this specific type.
- **Other Errors**: Standard I/O or permission errors where recovery depends on the specific error code.

Panics:
- **Allowed**: No. The interface defines a contract for returning errors via `Result`, not for panicking.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`MkNode`**: An asynchronous trait defining the `mk_node` method. It is marked `Send` via `trait_variant`, allowing it to be used as a trait object in multi-threaded async contexts.

---

## 7. Overview

This module is used in order to **define the contract for creating special file system nodes** (devices, sockets, and named pipes) within the `nfs_mamont` NFSv3 server. The system requires a specialized interface for this operation because creating these entities involves parameters not present in regular file creation (e.g., major/minor numbers for devices) and represents a distinct NFSv3 procedure (`MKNODE`) separate from `CREATE` (for regular files) or `MKDIR` (for directories).

A typical usage scenario of the system involves a client needing to create a device node (e.g., a character device for communication) or a FIFO for inter-process communication. The RPC layer receives the request, deserializes the arguments into the `Args` struct (which includes the `What` enum variant), and invokes the `mk_node` method on the VFS backend. The backend validates the request against its capabilities (returning `NotSupported` or `BadType` if necessary), creates the node with the specified attributes, and returns the `Success` struct containing the new file handle and `WccData`. This `WccData` is crucial because it allows the client to update its cache of the parent directory efficiently without performing a separate `READDIR` or `LOOKUP` call.

Inside the system, the following things happen and they use this module:
1. **Type-Safe Creation**: The `What` enum acts as a type-safe container for the heterogeneous arguments required for different special files. It ensures that device numbers are provided if and only if the type is `Char` or `Block`, preventing invalid states at compile time.
2. **Attribute Initialization**: By reusing `NewAttr` from the `set_attr` module, this module ensures that the logic for setting initial permissions, ownership, and timestamps is consistent between creating a new node and modifying an existing one.
3. **Protocol Compliance**: The `MkNode` trait enforces the NFSv3 requirement that the server must report whether it supports the operation generally (`NotSupported`) or just specific types (`BadType`), allowing clients to adapt their behavior based on server capabilities.

Without this module, the VFS layer would lack a standardized way to handle the creation of non-regular files, forcing the RPC layer to rely on ad-hoc interfaces or misusing the `CREATE` procedure, which would violate the NFSv3 specification and limit the functionality of the file server.