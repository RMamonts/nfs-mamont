<!-- SPEC_HASH: af02f1740d9f30daec120ec3e35320104c1bc76b3951645826e7f2c5d730bde4 -->
# Module Specification

Module: nfs_mamont::vfs::mk_node
Rust File: src/vfs/mk_node.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::vfs**: Used to import `vfs::DirOpArgs`, which defines the target directory and name for the new node. It is also used for `vfs::WccData` to provide Weak Cache Consistency information for the parent directory in both success and failure cases, and `vfs::Error` for reporting specific failure conditions such as unsupported operations or bad types.
- **super::file**: Used to import `file::Device` (for major/minor numbers in device creation), `file::Handle` (to identify the newly created file), and `file::Attr` (to return the attributes of the new file).
- **super::set_attr::NewAttr**: Used to specify the initial attributes (mode, uid, gid, size, timestamps) for the special file being created.
- **trait_variant::make**: Used to transform the `MkNode` trait into a `Send` trait object. This is necessary to allow the VFS implementation to be passed across thread boundaries in an asynchronous context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for creating special file system nodes (FIFOs, sockets, character devices, block devices) corresponding to the NFSv3 `MKNOD` procedure.
- To encapsulate the logic for handling device-specific parameters (major/minor numbers) alongside standard file attributes.
- To ensure that the implementation correctly reports support (or lack thereof) for specific special file types via specific error codes.

Inputs:
- `Args`: A structure containing:
  - `object`: A `vfs::DirOpArgs` identifying the parent directory handle and the new name for the node.
  - `what`: A `What` enum variant specifying the type of node to create (`Char`, `Block`, `Socket`, `Fifo`), along with the initial `NewAttr` and, for devices, the `file::Device` (major/minor numbers).

Outputs:
- `Result<Success, Fail>`:
  - `Success`: Contains the `file::Handle` and `file::Attr` of the created node, plus `vfs::WccData` for the directory.
  - `Fail`: Contains a `vfs::Error` and `vfs::WccData` for the directory.

Steps:
1. **Type Support Check**: The implementation determines if it supports the creation of special files in general. If not, it returns `vfs::Error::NotSupported`.
2. **Specific Type Check**: The implementation checks if it supports the specific type requested in `args.what` (e.g., character device vs. socket). If the type is unsupported, it returns `vfs::Error::BadType`.
3. **Parameter Extraction**: Depending on the `What` variant:
   - For `Char` or `Block`: Extracts the `NewAttr` and `file::Device` (major/minor).
   - For `Socket` or `Fifo`: Extracts the `NewAttr`.
4. **Node Creation**: The implementation creates the file system node within the directory specified by `args.object.dir` with the name `args.object.name`.
5. **Attribute Initialization**: The attributes specified in `NewAttr` are applied to the new node. For device files, the device ID is set using the provided major/minor numbers.
6. **WCC Data Collection**: The implementation captures the state of the parent directory before and after the operation to construct `vfs::WccData`.
7. **Result Construction**: On success, `Success` is populated with the new handle and attributes. On failure, `Fail` is populated with the error and the directory's pre-operation state (via WCC).

Edge Cases:
- **Unsupported Server**: If the server implementation does not support the `MKNOD` operation at all, `vfs::Error::NotSupported` is returned.
- **Unsupported Type**: If the server supports `MKNOD` but not the specific type (e.g., a filesystem that supports FIFOs but not block devices), `vfs::Error::BadType` is returned.

Complexity:
- Time: Dependent on the underlying file system implementation. The interface itself involves simple data structure access.
- Space: O(1) for the arguments and return structures.

Determinism:
- Deterministic (assuming the underlying file system state is stable).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs` (Assumption based on facts)**: `vfs::DirOpArgs` provides the context (parent directory handle and file name) required to locate where the new node should be created. `vfs::WccData` is critical for the NFS protocol's Weak Cache Consistency model, allowing the client to validate its cache of the directory contents without re-reading the entire directory. `vfs::Error` defines the specific error codes `NotSupported` and `BadType` which the implementation must use to signal capability constraints.
- **From `crate::vfs::file`**: `file::Device` provides the structure for major and minor numbers, which are essential for creating character and block device files. `file::Handle` and `file::Attr` are the standard VFS types used to identify and describe the resulting file object.
- **From `crate::vfs::set_attr`**: `NewAttr` is utilized here not to modify an existing file, but to define the initial metadata (permissions, ownership, timestamps) for the file at the moment of creation.

---

## 4. Data Model

Entities:
- **What**: A discriminated union (enum) identifying the type of special file.
  - `Char(NewAttr, file::Device)`: Character device.
  - `Block(NewAttr, file::Device)`: Block device.
  - `Socket(NewAttr)`: Socket.
  - `Fifo(NewAttr)`: Named pipe (FIFO).
- **Args**: The input arguments for the operation.
  - `object: vfs::DirOpArgs`: Location (directory + name).
  - `what: What`: Type and initial data.
- **Success**: The successful result wrapper.
  - `file: Option<file::Handle>`: Handle of the new node.
  - `attr: Option<file::Attr>`: Attributes of the new node.
  - `wcc_data: vfs::WccData`: Directory cache consistency data.
- **Fail**: The failure result wrapper.
  - `error: vfs::Error`: The specific error encountered.
  - `dir_wcc: vfs::WccData`: Directory cache consistency data.

Relations:
- **Composition**: `Args` aggregates `DirOpArgs` and `What`.
- **Composition**: `What` aggregates `NewAttr` and optionally `Device`.
- **Association**: `Success` and `Fail` both contain `vfs::WccData`.

Global Invariants:
- **Device Invariant**: The `Char` and `Block` variants of `What` must always contain a valid `file::Device`. The `Socket` and `Fifo` variants must not.

## 5. Error Model

Error Types:
- `vfs::Error`

Error Propagation Strategy:
- The method returns a `Result<Success, Fail>`. The `Fail` struct wraps the `vfs::Error` along with `vfs::WccData`.

Recoverability:
- Recoverable. The client receives the error code and the `WccData`, allowing it to synchronize its cache and understand why the creation failed (e.g., unsupported type).

Panics:
- Allowed: No
- Conditions: The trait definition does not specify panics. Implementations should return `vfs::Error` for expected failure modes.

---

## 6. Traits

List which external traits this module implements:
- **std::marker::Send**: Implemented for `MkNode` via the `#[trait_variant::make(Send)]` macro. This allows the trait object to be sent between threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the interface for creating special file system nodes (such as device files, named pipes, and sockets) within the `nfs_mamont` NFSv3 server implementation. The system contains a Virtual File System (VFS) layer that abstracts the underlying storage. While standard file creation is handled by other interfaces, the NFSv3 protocol specifically requires a `MKNOD` procedure to handle non-regular files and non-directories, particularly those requiring device major and minor numbers.

A typical usage scenario of the system involves an NFS client sending a `MKNOD` request to create a character device driver interface (e.g., `/dev/null`) or a FIFO for inter-process communication. The server decodes this request into the `Args` struct defined in this module. The `What` enum carries the specific type and the `NewAttr` carries the initial permissions and ownership.

Inside the system, the following things happen and they use this module: The VFS implementation uses the `MkNode` trait to delegate the creation logic to the storage backend. The backend must determine if it supports the requested node type. If the backend is a simple disk-based filesystem, it might support all types. If it is a network-based or object-based filesystem, it might return `vfs::Error::BadType` for device files. The `file::Device` struct is crucial here because it provides the strongly-typed major and minor numbers required by the underlying OS or filesystem format to correctly register the device. The result of the operation always includes `vfs::WccData`. This is essential for the client to update its directory cache efficiently, knowing whether the directory entry was added successfully or if the operation failed without modifying the directory. Without this module, the VFS would lack a standardized mechanism to handle the creation of these specific file types, breaking compatibility with POSIX-like environments that rely on device nodes and IPC pipes.