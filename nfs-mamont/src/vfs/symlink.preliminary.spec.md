<!-- SPEC_HASH: afb2f79fed336ad900abf0ac87edf4f7fcd2550ca735ad5c67e68993b871d842 -->
# Module Specification

Module: nfs_mamont::vfs::symlink
Rust File: src/vfs/symlink.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **crate::vfs**: Used to import `vfs::DirOpArgs`, `vfs::WccData`, and `vfs::Error`. `DirOpArgs` specifies the location (parent directory and name) for the new symbolic link. `vfs::WccData` is required in the return types to provide Weak Cache Consistency information for the parent directory. `vfs::Error` is used to report specific failure conditions, such as attempting to create a link named "." or "..".
- **super::file**: Used to import `file::Handle`, `file::Attr`, and `file::Path`. `file::Handle` and `file::Attr` are used in the `Success` return type to identify and describe the newly created link. `file::Path` is used in `Args` to specify the target path of the symbolic link.
- **super::set_attr**: Used to import `set_attr::NewAttr`. This type is used in `Args` to allow the caller to specify initial attributes (mode, uid, gid, etc.) for the symbolic link at the moment of creation.
- **trait_variant::make**: Used to transform the `Symlink` trait into a `Send` trait object. This is necessary to allow the implementation of the VFS to be passed across thread boundaries in an asynchronous context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for creating symbolic links within the Virtual File System (VFS) layer, corresponding to the NFSv3 `SYMLINK` procedure.
- To enforce specific naming constraints (prohibiting "." and "..") to maintain file system integrity.
- To guarantee atomicity during link creation, ensuring that the link node and its content (the target path) are visible to other operations simultaneously.
- To provide Weak Cache Consistency (WCC) data for the parent directory to allow clients to validate their directory caches.

Inputs:
- `Args`: A structure containing:
 - `object`: A `vfs::DirOpArgs` identifying the parent directory (`dir`) and the name (`name`) of the symbolic link to create.
 - `attr`: A `set_attr::NewAttr` structure specifying the initial attributes for the new link.
 - `path`: A `file::Path` representing the target path to which the symbolic link will point.

Outputs:
- `Result<Success, Fail>`:
 - `Success`: Contains the `file::Handle` of the new link, its `file::Attr`, and `vfs::WccData` for the parent directory.
 - `Fail`: Contains a `vfs::Error` describing the failure and `vfs::WccData` for the parent directory.

Steps:
1. **Name Validation**: The implementation checks the `name` field within `Args::object`.
 - If the name is "." or "..", the operation must fail immediately, returning `Err(Fail)` with `vfs::Error::Exist`.
2. **Atomic Creation**: The implementation creates the symbolic link node within the directory specified by `Args::object.dir`.
 - The creation of the file system node and the writing of the target path (`Args::path`) into the node must occur atomically. There must be no observable state where the link exists but `read_link` would fail or return incorrect data.
3. **Attribute Initialization**: If fields in `Args::attr` are set, the implementation applies these attributes to the new node.
4. **WCC Data Collection**: The implementation captures the state of the parent directory before and after the operation to construct `vfs::WccData`.
5. **Result Return**: The method returns `Ok(Success)` with the new handle and attributes, or `Err(Fail)` if an error occurred.

Edge Cases:
- **Reserved Names**: Attempting to create a link named "." or ".." is explicitly forbidden and results in `vfs::Error::Exist`.
- **Atomicity Requirement**: The underlying file system must support atomic creation of the link and its content. If the FS cannot guarantee this, it does not comply with this interface's contract.

Complexity:
- Time: Dependent on the underlying file system implementation. The interface itself involves simple data structure access.
- Space: O(1) for the arguments and return structures.

Determinism:
- Deterministic (assuming the underlying file system state is stable).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs` (Assumption based on facts)**: `vfs::DirOpArgs` provides the context for the operation (parent directory handle and entry name). `vfs::WccData` is a struct containing `before: Option<file::WccAttr>` and `after: Option<file::Attr>`, which is critical for the client-side caching strategy. `vfs::Error` defines the error codes, specifically `Exist` (value 17), which is used for the "." and ".." check.
- **From `crate::vfs::file`**: `file::Handle` acts as the unique identifier for the new link. `file::Attr` aggregates the metadata (type, mode, timestamps, etc.) that must be returned upon successful creation. `file::Path` wraps the target string, ensuring it meets VFS length constraints.
- **From `crate::vfs::set_attr`**: `set_attr::NewAttr` allows the caller to pass a set of optional attributes (mode, uid, gid, size, atime, mtime) to be applied to the new link. This allows the client to set permissions or ownership immediately upon creation.

---

## 4. Data Model

Entities:
- **Args**: The input arguments for the `symlink` operation.
 - `pub object: vfs::DirOpArgs`
 - `pub attr: super::set_attr::NewAttr`
 - `pub path: file::Path`
- **Success**: The successful result wrapper.
 - `pub file: Option<file::Handle>`
 - `pub attr: Option<file::Attr>`
 - `pub wcc_data: vfs::WccData`
- **Fail**: The failure result wrapper.
 - `pub error: vfs::Error`
 - `pub dir_wcc: vfs::WccData`

Relations:
- **Composition**: `Args` aggregates `DirOpArgs`, `NewAttr`, and `Path`.
- **Association**: `Success` and `Fail` both contain `vfs::WccData`.

Global Invariants:
- **Naming Invariant**: If `Args::object.name` is "." or "..", the implementation MUST return `vfs::Error::Exist`.
- **Atomicity Invariant**: The symbolic link node and its contents (the target path) must be created in a single atomic operation. Once the link is visible in the directory, a `read_link` operation on it must succeed and return the correct path.

## 5. Error Model

Error Types:
- `vfs::Error`

Error Propagation Strategy:
- The method returns a `Result<Success, Fail>`. The `Fail` struct wraps the `vfs::Error` along with `vfs::WccData` for the directory.

Recoverability:
- Recoverable. The client receives the error code and the `WccData` (which contains the pre-operation attributes in `before`), allowing it to synchronize its cache.

Panics:
- Allowed: No
- Conditions: The trait definition does not specify panics. Implementations should return `vfs::Error` for expected failure modes.

---

## 6. Traits

List which external traits this module implements:
- **std::marker::Send**: Implemented for `Symlink` via the `#[trait_variant::make(Send)]` macro. This allows the trait object to be sent between threads.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the interface for creating symbolic links within the `nfs_mamont` NFSv3 server implementation. The system contains a Virtual File System (VFS) layer that abstracts the underlying storage (e.g., local disk, network storage) from the NFS protocol logic. A typical usage scenario involves an NFS client sending a `SYMLINK` request to create a pointer from one path to another. The server decodes this request into the `Args` struct defined in this module and invokes the `symlink` method on the VFS implementation.

Inside the system, the following things happen and they use this module: The VFS implementation uses the `Args` struct to determine the parent directory and the name of the new link, as well as the target path stored in the link. The module enforces a strict naming rule: names "." and ".." are rejected with `vfs::Error::Exist` to prevent directory traversal confusion or corruption. Crucially, the module mandates an atomic creation process. This means that the file system must not make the link visible to other operations (like `lookup` or `read_link`) until the link content is fully written. This atomicity is vital for NFSv3 compliance to prevent race conditions where a client might try to read a link immediately after creation and find it empty or invalid. The result of the operation, whether success or failure, includes `vfs::WccData` for the parent directory. This data structure is essential for the NFS protocol's Weak Cache Consistency model, allowing the client to validate or invalidate its cached directory contents without performing a full `READDIR` call. Without this module, the VFS would lack a standardized way to handle symbolic link creation, atomicity guarantees, and the specific WCC requirements of the NFSv3 protocol.