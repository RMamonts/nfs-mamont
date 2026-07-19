<!-- SPEC_HASH: 282e1c29d4eab474bedae8331053d0b469f5f9fc941e986f4497f8ee1193db2d -->
# Module Specification

Module: nfs_mamont::vfs::create
Rust File: src/vfs/create.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::consts::nfsv3`**: Used to import `NFS3_CREATEVERFSIZE`. This constant defines the fixed size of the `Verifier` byte array, ensuring compliance with the NFSv3 protocol specification for exclusive file creation.
- **`crate::vfs`**: Used to import `vfs::WccData` and `vfs::Error`. `WccData` is required in the return types (`Success` and `Fail`) to provide Weak Cache Consistency information for the directory where the file is created. `vfs::Error` is used to report specific failure conditions, such as `Exist` when a duplicate file is detected in `Guarded` mode.
- **`crate::vfs::file`**: Used to import `file::Handle` and `file::Attr`. These types are used in the `Success` struct to return the identifier and metadata of the newly created file. The documentation also references `file::Type::Regular` to specify the type of file being created.
- **`crate::vfs::set_attr`**: Used to import `set_attr::NewAttr`. This type is used within the `How` enum (`Unchecked` and `Guarded` variants) to specify the initial attributes (mode, uid, gid, size, etc.) for the file being created.
- **`trait_variant::make`**: Used to transform the `Create` trait into a `Send` trait object. This is necessary to allow the implementation of the VFS to be passed across thread boundaries in an asynchronous context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for creating a regular file within the Virtual File System (VFS) layer, strictly adhering to the semantics of the NFSv3 `CREATE` procedure.
- To support three distinct creation strategies defined by the NFSv3 protocol: `Unchecked` (create or truncate), `Guarded` (create only if it doesn't exist), and `Exclusive` (create atomically using a verifier).
- To encapsulate the arguments and results of the creation operation, including Weak Cache Consistency (WCC) data for the parent directory.

Inputs:
- `Args`: A structure containing:
 - `object`: A `vfs::DirOpArgs` identifying the directory (via `file::Handle`) and the name of the file to create.
 - `how`: A `How` enum variant specifying the creation mode and associated data (attributes or verifier).

Outputs:
- `Result<Success, Fail>`:
 - `Success`: Contains the `file::Handle` and `file::Attr` of the created file, along with `vfs::WccData` for the directory.
 - `Fail`: Contains a `vfs::Error` describing the failure and `vfs::WccData` for the directory.

Steps:
1. **Mode Dispatch**: The implementation of `Create::create` inspects the `args.how` field to determine the creation strategy.
2. **Unchecked Mode**:
 - The implementation attempts to create the file with the attributes specified in `How::Unchecked`.
 - If the file already exists, the behavior typically follows NFSv3 semantics: the file is truncated to zero length, and its attributes are set to the provided values (though the specific behavior depends on the VFS backend implementation).
3. **Guarded Mode**:
 - The implementation checks for the existence of a file with the specified name in the target directory.
 - If the file exists, the operation must fail immediately, returning `Err(Fail)` with `error` set to `vfs::Error::Exist`.
 - If the file does not exist, the operation proceeds as in `Unchecked` mode.
4. **Exclusive Mode**:
 - The implementation uses the provided `Verifier` to ensure exclusive creation.
 - If the file does not exist, it is created. The server may store the verifier in the file's metadata (e.g., `mtime` or `ctime`) to verify future requests.
 - If the file exists, the implementation checks if the stored verifier matches the provided one. If it matches, the operation typically succeeds (indicating a retry of a successful request). If it does not match, the operation fails (e.g., with `vfs::Error::Exist`).
5. **Attribute Application**: For `Unchecked` and `Guarded` modes, the attributes provided in `NewAttr` are applied to the file. If `size` is set to 0, the file is truncated.
6. **WCC Data Collection**: The implementation captures the attributes of the parent directory before the operation (if available) and after the operation to populate `vfs::WccData`.
7. **Result Construction**: The method returns `Ok(Success)` with the new file's handle and attributes, or `Err(Fail)` with the error and directory WCC data.

Edge Cases:
- **Verifier Storage**: The protocol allows the server to store the `Verifier` in the file's metadata (e.g., `mtime`) during `Exclusive` creation. The exact mechanism is opaque to the client but must be consistent by the server.
- **Attribute Limits**: If the provided `NewAttr` contains values that the underlying file system cannot support (e.g., UID/GID too large), the operation should fail with an appropriate `vfs::Error`.

Complexity:
- Time: Dependent on the underlying file system implementation (directory lookup, inode allocation, metadata updates).
- Space: O(1) for the arguments and return structures.

Determinism:
- Deterministic (assuming the underlying file system state is stable).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs` (Assumption based on facts)**: `vfs::WccData` is a struct containing `before: Option<file::WccAttr>` and `after: Option<file::Attr>`. This mechanism is critical for the `Success` and `Fail` return types to allow the client to verify the state of the directory cache. `vfs::Error` is an enum defining specific error codes like `Exist` (value 17), which is explicitly required for the `Guarded` mode logic.
- **From `crate::vfs::file`**: `file::Handle` is a fixed-size byte array `[u8; NFS3_FHSIZE]` acting as an opaque identifier for the file. `file::Attr` aggregates metadata fields (type, mode, uid, gid, size, timestamps) which are returned to the client upon successful creation.
- **From `crate::vfs::set_attr`**: `set_attr::NewAttr` is a struct collecting optional attribute changes (`mode`, `uid`, `gid`, `size`, `atime`, `mtime`). This mechanism allows the `Create` interface to initialize the file with specific metadata in a single operation, rather than requiring a separate `SETATTR` call.

---

## 4. Data Model

Entities:
- **Verifier**: A newtype wrapper around a fixed-size byte array `[u8; NFS3_CREATEVERFSIZE]`. It acts as an opaque token used in `Exclusive` creation mode to guarantee that the file is created by the specific client request.
- **How**: An enumeration describing the creation strategy.
 - `Unchecked(NewAttr)`: Create or truncate.
 - `Guarded(NewAttr)`: Create only if non-existent.
 - `Exclusive(Verifier)`: Create atomically with a verifier check.
- **HowMode**: An enumeration providing integer discriminants for the `How` variants (0, 1, 2), likely used for serialization or protocol matching.
- **Args**: The input arguments for the `create` operation.
 - `object`: `vfs::DirOpArgs` (directory handle + filename).
 - `how`: `How` (creation strategy).
- **Success**: The successful result wrapper.
 - `file`: `Option<file::Handle>` (handle of the new file).
 - `attr`: `Option<file::Attr>` (attributes of the new file).
 - `wcc_data`: `vfs::WccData` (directory cache consistency data).
- **Fail**: The failure result wrapper.
 - `error`: `vfs::Error`.
 - `wcc_data`: `vfs::WccData`.

Relations:
- **Composition**: `Args` aggregates `vfs::DirOpArgs` and `How`.
- **Association**: `Success` and `Fail` both contain `vfs::WccData`.
- **Dependency**: `How` variants depend on `set_attr::NewAttr` or `Verifier`.

Global Invariants:
- **Verifier Size**: The `Verifier` array is always exactly `NFS3_CREATEVERFSIZE` bytes.
- **File Type**: The `create` operation is strictly defined to create a `file::Type::Regular` file. Implementations must not create directories or special files via this interface.

## 5. Error Model

Error Types:
- `vfs::Error`

Error Propagation Strategy:
- The method returns a `Result<Success, Fail>`. The `Fail` struct wraps the `vfs::Error` along with `vfs::WccData`.

Recoverability:
- Recoverable. The client receives the error code and the `WccData` (which contains the pre-operation attributes in `before`), allowing it to synchronize its cache and potentially retry the operation (e.g., if `Exist` was returned in `Guarded` mode, the client knows the file is there).

Panics:
- Allowed: No
- Conditions: The trait definition does not specify panics. Implementations should return `vfs::Error` for expected failure modes.

---

## 6. Traits

List which external traits this module implements:
- **std::marker::Send**: Implemented for `Create` via the `#[trait_variant::make(Send)]` macro. This allows the trait object to be sent between threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the interface for creating regular files within the `nfs_mamont` NFSv3 server implementation. The system contains a Virtual File System (VFS) layer that abstracts the underlying storage (e.g., local disk, network storage) from the NFS protocol logic. A typical usage scenario involves an NFS client sending a `CREATE` request to create a new file or truncate an existing one. The server decodes this request into the `Args` struct defined in this module and invokes the `create` method on the VFS implementation.

Inside the system, the following things happen and they use this module: The VFS implementation uses the `How` enum to determine the specific semantics of the creation operation. The `Guarded` variant is crucial for clients that want to ensure they do not overwrite existing data, forcing the VFS to check for existence and return `vfs::Error::Exist` if the file is already present. The `Exclusive` variant provides a mechanism for safe, atomic file creation in distributed environments by using a `Verifier` to handle race conditions where multiple clients might try to create the same file simultaneously. The result of the operation, whether success or failure, always includes `vfs::WccData`. This data structure is essential for the NFS protocol's Weak Cache Consistency model, allowing the client to validate or invalidate its cached directory data without performing a full `READDIR` call. Without this module, the VFS would lack a standardized way to handle the specific creation modes and WCC requirements of the NFSv3 protocol, leading to potential data corruption or cache coherency issues.