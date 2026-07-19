<!-- SPEC_HASH: fec8bfaf04675d9133c5b51699d6d820b5f785d3f4aaf2b668e16f5de0b88de5 -->
# Module Specification

Module: nfs_mamont::vfs
Rust File: src/vfs/mod.rs

---

## 1. Dependencies

From the source code and context, for each dependency, write down its purpose — why it is used in this module.

- **`num_derive`**: Used to derive `FromPrimitive` and `ToPrimitive` traits for the `Error` enum. This allows conversion between the enum variants and integer discriminants, which is necessary for mapping to and from the NFSv3 wire protocol status codes.
- **`crate::allocator::Buffer`**: Used as a generic bound (`B: Buffer`) for the `Vfs` trait and the `NfsRes` enum. This abstracts the memory management strategy, allowing the VFS to interact with data buffers allocated by the server's custom allocator without knowing the concrete implementation.
- **Sub-modules (`access`, `commit`, `create`, `file`, `fs_info`, `fs_stat`, `get_attr`, `link`, `lookup`, `mk_dir`, `mk_node`, `path_conf`, `read`, `read_dir`, `read_dir_plus`, `read_link`, `remove`, `rename`, `rm_dir`, `set_attr`, `symlink`, `write`)**: These modules define the specific traits and data structures for individual NFSv3 procedures. They are re-exported and aggregated by this module to form the complete `Vfs` interface and the `NfsRes` result enum.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the central aggregation point for the Virtual File System (VFS) layer, unifying all NFSv3 file system operations under a single trait (`Vfs`).
- To provide a standardized set of error codes (`Error`) that map to NFSv3 protocol statuses and server-specific conditions.
- To define common data structures (`WccData`, `DirOpArgs`) used across multiple file system operations to ensure consistency in Weak Cache Consistency (WCC) handling and directory operation arguments.
- To provide a unified result type (`NfsRes`) that can encapsulate the outcome of any supported NFSv3 procedure, facilitating generic dispatch and handling logic in the RPC layer.

Inputs:
- Generic type `B`: A type implementing the `Buffer` trait, representing the memory management strategy used for data transfer (read/write operations).

Outputs:
- **`Vfs<B>` Trait**: A super-trait combining all specific operation traits (e.g., `Read`, `Write`, `Lookup`).
- **`Error` Enum**: A comprehensive enumeration of error conditions, including standard NFSv3 errors (e.g., `NoEntry`, `IO`) and implementation-specific errors (e.g., `JUKEBOX`, `BadCookie`).
- **`WccData` Struct**: A structure holding optional pre-operation (`before`) and post-operation (`after`) file attributes.
- **`DirOpArgs` Struct**: A structure holding a directory handle and an entry name, used as arguments for directory modification operations.
- **`NfsRes<B>` Enum**: An enumeration where each variant corresponds to an NFSv3 procedure, wrapping the specific `Result<Success, Fail>` type from the respective sub-module.

Steps:
1. **Error Definition**: The `Error` enum is defined with specific discriminant values (e.g., `NoEntry = 2`) to match the NFSv3 protocol specification. It includes custom errors (values > 10000) for server-specific logic like `BadCookie` or `JUKEBOX`.
2. **Common Structure Definition**: `WccData` is defined to hold `Option<file::WccAttr>` and `Option<file::Attr>`. `DirOpArgs` is defined to hold `file::Handle` and `file::Name`.
3. **Trait Aggregation**: The `Vfs<B>` trait is defined with a super-trait bound requiring the implementation of all specific operation traits (e.g., `get_attr::GetAttr`, `read::Read<B>`, etc.).
4. **Blanket Implementation**: A generic implementation `impl<T, B> Vfs<B> for T` is provided. This allows any type `T` that implements all required sub-traits to automatically satisfy the `Vfs<B>` contract.
5. **Result Unification**: The `NfsRes<B>` enum is defined with variants for every procedure (e.g., `Read(Result<read::Success<B>, read::Fail>)`). This allows the RPC layer to return a single enum type regardless of which procedure was executed.

Edge Cases:
- **`NfsRes::Null`**: Represents a procedure with no return value. While most NFSv3 procedures return data, this variant exists for completeness or specific internal procedures.
- **Custom Errors**: The `Error` enum includes values like `JUKEBOX` (10008), which indicates the server is busy processing a file migration (e.g., from HSM to disk), a behavior specific to this implementation's handling of state transitions.

Complexity:
- **Time**: O(1) for defining the types and traits. The complexity of the operations themselves is delegated to the sub-modules.
- **Space**: O(1) for the structures defined in this module.

Determinism:
- **Deterministic**: The module defines types and interfaces; there is no runtime logic in this module itself.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: Used in `DirOpArgs` to identify the target directory for directory operations.
 - **`Name`**: Used in `DirOpArgs` to specify the name of the entry within the directory.
 - **`Attr` and `WccAttr`**: Used in `WccData` to represent the post-operation and pre-operation attributes of a file system object, respectively. This is critical for the Weak Cache Consistency mechanism used throughout the NFSv3 protocol.

- **From `nfs_mamont::vfs::<submodules>` (e.g., `read`, `write`, `lookup`, etc.)**:
 - **Specific Traits (e.g., `Read`, `Write`)**: These traits are aggregated into the `Vfs` super-trait. The `Vfs` trait acts as a facade, requiring the implementer to support all these individual capabilities.
 - **Result Types (e.g., `read::Success`, `read::Fail`)**: These are wrapped in the `NfsRes` enum variants. This allows the RPC layer to dispatch a request and receive a typed result without knowing the specific procedure at compile time (via the enum).

- **From `nfs_mamont::allocator`**:
 - **`Buffer` Trait**: Used as the generic bound `B` in `Vfs<B>` and `NfsRes<B>`. This decouples the VFS interface from the specific memory allocation strategy of the server, allowing the VFS to read/write data into buffers managed by the server's pool.

---

## 4. Data Model

Entities:
- **`Error`**: An enumeration of error codes.
 - Includes standard NFSv3 errors (e.g., `Permission`, `NoEntry`, `IO`).
 - Includes implementation-specific errors (e.g., `BadFileHandle`, `NotSync`, `BadCookie`, `JUKEBOX`).
- **`WccData`**: A structure for Weak Cache Consistency data.
 - `before`: `Option<file::WccAttr>` (Attributes before the operation).
 - `after`: `Option<file::Attr>` (Attributes after the operation).
- **`DirOpArgs`**: Arguments for directory operations.
 - `dir`: `file::Handle` (The directory handle).
 - `name`: `file::Name` (The entry name).
- **`Vfs<B: Buffer>`**: A trait combining all specific VFS operation traits.
- **`NfsRes<B: Buffer>`**: An enum wrapping the result of any NFSv3 procedure.
 - Variants include `Null`, `GetAttr`, `SetAttr`, `LookUp`, `Access`, `ReadLink`, `Read`, `Write`, `Create`, `MkDir`, `SymLink`, `MkNod`, `Remove`, `RmDir`, `Rename`, `Link`, `ReadDir`, `ReadDirPlus`, `FsStat`, `FsInfo`, `PathConf`, `Commit`.

Relations:
- **Aggregation**: `Vfs<B>` aggregates all specific operation traits (e.g., `Read<B>`, `Write<B>`).
- **Composition**: `WccData` composes `file::WccAttr` and `file::Attr`.
- **Composition**: `DirOpArgs` composes `file::Handle` and `file::Name`.
- **Association**: `NfsRes<B>` variants are associated with the `Result` types of the specific sub-modules.

Global Invariants:
- **Error Codes**: The discriminant values of `Error` variants are fixed and correspond to specific protocol or implementation-defined constants (e.g., `STATUS_OK` is 0).
- **WCC Availability**: Operations that modify directory state typically return `WccData` to allow clients to update their caches.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error enumeration defined in this module. It covers both standard NFSv3 protocol errors and server-specific errors.

Error Propagation Strategy:
- **Centralized Enum**: All sub-modules utilize `vfs::Error` within their `Fail` structs. This ensures a single source of truth for error codes across the entire VFS layer.
- **Result Wrapping**: The `NfsRes` enum wraps the `Result<Success, Fail>` from sub-modules. If a sub-module operation fails, the `Err(Fail)` variant is used, which contains the `vfs::Error`.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `vfs::Error` variant. For example, `IO` might be transient (recoverable), while `StaleFile` implies a persistent state issue requiring the client to re-lookup the file.

Panics:
- **Allowed**: No. The `Error` enum and related structures are designed for data transfer and error reporting, not for panicking.

---

## 6. Traits

List which external traits this module implements:
- **`Vfs<B>` for `T`**: The module provides a blanket implementation `impl<T, B> Vfs<B> for T` where `B: Buffer` and `T` implements all required sub-traits. This allows any compliant backend to be used as a `Vfs` object automatically.

List which traits this module defines:
- **`Vfs<B: Buffer>`**: The main trait defining the complete Virtual File System interface.

---

## 7. Overview

This module is used in order to **unify and aggregate the entire Virtual File System (VFS) interface** for the `nfs_mamont` NFSv3 server. The system contains a complex architecture where the network layer (RPC handling) is separated from the storage layer (VFS implementation). A typical usage scenario of the system involves the main server loop receiving an NFSv3 request. The RPC dispatcher identifies the procedure type (e.g., `READ`, `WRITE`, `LOOKUP`) and invokes the corresponding method on the `Vfs` trait object. The result, regardless of the procedure, is wrapped in the `NfsRes` enum, which is then serialized and sent back to the client.

Inside the system, the following things happen and they use this module:
1. **Interface Unification**: The `Vfs` trait acts as a facade. It requires that any storage backend plugged into the server implements *all* supported NFSv3 operations (e.g., `Read`, `Write`, `Lookup`, `Commit`). This ensures that the server can handle any valid NFSv3 request without runtime checks for capability support.
2. **Standardized Error Handling**: The `Error` enum defined here is used by every sub-module. When a `read` operation fails, it returns a `Fail` struct containing `vfs::Error::IO`. When a `lookup` fails, it might return `vfs::Error::NoEntry`. This centralization allows the RPC layer to map any error to the correct NFS status code consistently.
3. **Generic Dispatch**: The `NfsRes` enum allows the RPC layer to handle results generically. The dispatcher can match on `NfsRes` to extract the specific success or failure data without needing to know the specific procedure logic, facilitating a clean separation of concerns.
4. **Cache Consistency**: The `WccData` struct, defined here and used by sub-modules, ensures that every operation modifying file or directory state returns the necessary attributes (before and after) to support the NFSv3 Weak Cache Consistency model, allowing clients to validate their cached data efficiently.

Without this module, the server would lack a single entry point for the file system, forcing the RPC layer to manage dozens of independent traits and result types. This would lead to code duplication, inconsistent error handling, and a fragile architecture where adding a new procedure would require changes in multiple places. This module encapsulates the complexity of the NFSv3 protocol into a coherent, type-safe interface.