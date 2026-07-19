<!-- SPEC_HASH: fec8bfaf04675d9133c5b51699d6d820b5f785d3f4aaf2b668e16f5de0b88de5 -->
# Module Specification

Module: nfs_mamont::vfs
Rust File: src/vfs/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`num_derive`**: Used to derive `FromPrimitive` and `ToPrimitive` traits for the `Error` enum. This allows error codes to be easily converted to and from integer representations, which is necessary for mapping to the NFSv3 wire protocol status codes.
- **`crate::allocator::Buffer`**: Used as a generic bound `B` for the `Vfs` trait and the `NfsRes` enum. This abstracts the memory management strategy, allowing the VFS to operate on buffers allocated by the server's custom allocator without knowing the concrete implementation.
- **`crate::vfs::file`**: Used to import core domain types: `Handle`, `Name`, `Attr`, and `WccAttr`. These types are used within `WccData` and `DirOpArgs` to represent file identities, validated names, metadata, and cache consistency attributes.
- **Sub-modules (`access`, `commit`, `create`, `file`, `fs_info`, `fs_stat`, `get_attr`, `link`, `lookup`, `mk_dir`, `mk_node`, `path_conf`, `read`, `read_dir`, `read_dir_plus`, `read_link`, `remove`, `rename`, `rm_dir`, `set_attr`, `symlink`, `write`)**: These modules define the specific traits for each NFSv3 procedure (e.g., `Read`, `Write`, `Lookup`). The `mod.rs` module aggregates these traits into the main `Vfs` super-trait, ensuring that any implementation of `Vfs` supports the full suite of NFS operations.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: The `Vfs` Super-Trait

**Intent:**
To define a single, aggregate interface that represents a complete NFSv3 file system implementation. By combining all specific procedure traits (like `Read`, `Write`, `Lookup`), it allows the upper layers of the server (RPC handling) to interact with a storage backend using a single type (`dyn Vfs`) rather than managing disjoint traits for every operation.

**Inputs:**
- None (it is a trait definition).

**Outputs:**
- A trait object `dyn Vfs<B>` that exposes methods for all NFSv3 procedures.

**Steps:**
1. The `Vfs<B: Buffer>` trait is defined with a list of super-trait bounds (e.g., `get_attr::GetAttr`, `set_attr::SetAttr`, `read::Read<B>`, etc.).
2. A blanket implementation `impl<T, B> Vfs<B> for T` is provided where `T` implements all the required super-traits. This allows any type `T` that implements the specific operations to automatically be considered a `Vfs`.

**Edge Cases:**
- **Generic Buffer**: The `Vfs` trait is generic over `B: Buffer`. This means the specific buffer type (e.g., a pooled slice) is determined by the caller/context, allowing the same VFS logic to be used with different memory allocators.

**Complexity:**
- Time: O(1) (trait resolution).
- Space: O(1).

**Determinism:**
- Deterministic (trait definition).

### Mechanism 2: The `Error` Enumeration

**Intent:**
To provide a centralized, exhaustive list of error conditions that can occur within the VFS layer. This enum maps directly to NFSv3 status codes (e.g., `NFS3ERR_IO`, `NFS3ERR_NOENT`) and includes server-specific extensions (e.g., `BadFileHandle`, `NotSync`).

**Inputs:**
- None (enum variants).

**Outputs:**
- An `Error` enum variant representing a specific failure condition.

**Steps:**
1. The enum is defined with variants corresponding to standard NFS errors (discriminants 1-71) and custom errors (discriminants 10001+).
2. Derives `Debug`, `Copy`, `Clone`, `PartialEq`, `Eq`, `ToPrimitive`, and `FromPrimitive` for easy integration with logging and network serialization.

**Edge Cases:**
- **Server Fault**: The `ServerFault` variant (10006) acts as a catch-all for errors that do not map to specific NFS codes, instructing the client to treat it as a generic I/O error.

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

### Mechanism 3: `WccData` (Weak Cache Consistency Data)

**Intent:**
To define a standard structure for returning pre- and post-operation attributes of a file system object. This is critical for the NFSv3 Weak Cache Consistency model, allowing clients to validate their cached data without re-reading the entire file or directory.

**Inputs:**
- `before: Option<file::WccAttr>` (Attributes before the operation).
- `after: Option<file::Attr>` (Attributes after the operation).

**Outputs:**
- A `WccData` struct instance.

**Steps:**
1. The struct is defined with two optional fields.
2. It is used in the `Success` and `Fail` types of various sub-module traits (e.g., `set_attr::Success`, `remove::Fail`).

**Edge Cases:**
- **Partial Availability**: The fields are `Option` to handle cases where attributes could not be retrieved (e.g., if the object was deleted or the server was unable to read them).

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

### Mechanism 4: `DirOpArgs` (Directory Operation Arguments)

**Intent:**
To provide a common structure for arguments passed to directory-modifying operations (like `CREATE`, `MKDIR`, `REMOVE`, `RENAME`). It encapsulates the target directory handle and the entry name.

**Inputs:**
- `dir: file::Handle` (The directory handle).
- `name: file::Name` (The entry name).

**Outputs:**
- A `DirOpArgs` struct instance.

**Steps:**
1. The struct is defined aggregating a `Handle` and a `Name`.
2. It is used as a field in `Args` structs within sub-modules (e.g., `create::Args`, `remove::Args`).

**Edge Cases:**
- **Validation**: The `file::Name` type ensures the name is valid (length, no separators) before it reaches the operation logic.

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

### Mechanism 5: `NfsRes` Enum (Result Wrapper)

**Intent:**
To provide a single enum type that can hold the result of *any* NFSv3 procedure. This is useful for the RPC layer to handle results generically, dispatching based on the procedure type, or for internal routing where the specific procedure context might be abstracted away.

**Inputs:**
- `Result<Success, Fail>` from various sub-modules (e.g., `get_attr::Success`, `read::Success<B>`).

**Outputs:**
- An `NfsRes<B>` enum variant.

**Steps:**
1. The enum is defined with a variant for each NFS procedure (e.g., `GetAttr`, `Read`, `Write`).
2. Each variant wraps the corresponding `Result` type from the specific sub-module.

**Edge Cases:**
- **Null Variant**: Includes a `Null` variant, likely used for procedures that return no data (though NFSv3 NULL is a specific procedure, the presence here suggests a generic placeholder or unused state).

**Complexity:**
- Time: O(1).
- Space: O(1) + size of the contained result.

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: Used in `DirOpArgs` to identify the target directory. The `Vfs` layer relies on this opaque identifier to reference file system objects without exposing internal paths.
 - **`Name`**: Used in `DirOpArgs` to represent the target entry name. The `Vfs` layer relies on the validation guarantees provided by the `Name` type (e.g., no path separators).
 - **`Attr` and `WccAttr`**: Used in `WccData` to represent the state of the file system object before and after an operation. This is the core data payload for the Weak Cache Consistency mechanism.

- **From Sub-modules (e.g., `read`, `write`, `lookup`)**:
 - **Trait Definitions**: The `Vfs` trait aggregates the specific traits defined in these modules (e.g., `Read`, `Write`, `Lookup`). The `mod.rs` module acts as the facade, collecting these disparate interfaces into a single contract (`Vfs`) that the rest of the application can depend on.
 - **Result Types**: The `NfsRes` enum wraps the `Result` types (e.g., `read::Result<Success, Fail>`) defined in these modules, allowing them to be handled uniformly.

- **From `nfs_mamont::allocator`**:
 - **`Buffer` Trait**: Used as the generic bound `B` for the `Vfs` trait. This decouples the VFS logic from the specific memory implementation, allowing the server to enforce memory pooling strategies via the `Buffer` interface.

---

## 4. Data Model

Entities:
- **`Error`**: An enumeration of error codes.
 - Variants: `Permission`, `NoEntry`, `IO`, `NXIO`, `Access`, `Exist`, `XDev`, `NoDev`, `NotDir`, `IsDir`, `InvalidArgument`, `FileTooLarge`, `NoSpace`, `ReadOnlyFs`, `TooManyLinks`, `NameTooLong`, `NotEmpty`, `QuotaExceeded`, `StaleFile`, `TooManyLevelsOfRemote`, `BadFileHandle`, `NotSync`, `BadCookie`, `NotSupported`, `TooSmall`, `ServerFault`, `BadType`, `JUKEBOX`.
- **`WccData`**: A structure for Weak Cache Consistency data.
 - Fields: `before: Option<file::WccAttr>`, `after: Option<file::Attr>`.
- **`DirOpArgs`**: Arguments for directory operations.
 - Fields: `dir: file::Handle`, `name: file::Name`.
- **`Vfs<B: Buffer>`**: A super-trait combining all NFSv3 procedure traits.
- **`NfsRes<B: Buffer>`**: An enum wrapping all possible procedure results.

Relations:
- **Composition**: `WccData` composes `file::WccAttr` and `file::Attr`.
- **Composition**: `DirOpArgs` composes `file::Handle` and `file::Name`.
- **Aggregation**: `Vfs` aggregates traits from all sub-modules (`GetAttr`, `SetAttr`, `Lookup`, etc.).
- **Aggregation**: `NfsRes` aggregates `Result` types from all sub-modules.

Global Invariants:
- **Error Mapping**: The discriminants of the `Error` enum variants 1-71 must match the standard NFSv3 protocol error codes.
- **WCC Semantics**: In `WccData`, if `before` is `Some`, it represents the state strictly before the operation. If `after` is `Some`, it represents the state strictly after the operation.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The primary error type for the VFS layer. It encompasses standard NFS errors (e.g., `IO`, `NoEntry`) and implementation-specific errors (e.g., `BadFileHandle`, `NotSync`).

Error Propagation Strategy:
- **Trait Results**: All sub-module traits (e.g., `Read`, `Write`) return `Result<Success, Fail>`, where `Fail` contains a `vfs::Error`.
- **Centralization**: By defining `Error` in the parent `vfs` module, all sub-modules use a consistent error type, allowing the RPC layer to perform a single mapping from `vfs::Error` to wire status codes.

Recoverability:
- **Dependent on Variant**:
 - `IO`, `NXIO`, `JUKEBOX`: Potentially transient/retryable.
 - `StaleFile`, `BadFileHandle`: Requires the client to perform a new lookup.
 - `Access`, `Permission`: Requires the client to change permissions or credentials.
 - `Exist`, `NoEntry`: Indicates the requested state conflicts with reality; retrying with same arguments is futile.

Panics:
- **Allowed**: No. The `Error` enum is designed to be returned via `Result`, not to be used for panics.

---

## 6. Traits

List which external traits this module implements:
- **`FromPrimitive`**: Implemented for `Error` (via `num_derive`).
- **`ToPrimitive`**: Implemented for `Error` (via `num_derive`).

List which traits this module defines:
- **`Vfs<B: Buffer>`**: The super-trait defining the complete NFSv3 interface. It requires the implementation of all specific procedure traits (`GetAttr`, `SetAttr`, `Lookup`, `Access`, `ReadLink`, `Read<B>`, `Write<B>`, `Create`, `MkDir`, `Symlink`, `MkNode`, `Remove`, `RmDir`, `Rename`, `Link`, `ReadDir`, `ReadDirPlus`, `FsStat`, `FsInfo`, `PathConf`, `Commit`).

---

## 7. Overview

This module is used in order to **define the root contract and shared vocabulary for the Virtual File System (VFS) layer** within the `nfs_mamont` NFSv3 server. The system contains a complex architecture where the storage backend is abstracted behind a multitude of specific traits (one for each NFS procedure like `READ`, `WRITE`, `LOOKUP`). This module is necessary because it acts as the "facade" that unifies these disparate interfaces into a single, coherent entity (`Vfs`) that the rest of the server (RPC handling, connection management) can depend on. Without this module, the upper layers would need to manage dozens of independent trait objects or make assumptions about the backend's capabilities.

A typical usage scenario of the system involves the server startup logic in `lib.rs`. The server constructs a storage backend object (e.g., a wrapper around a local filesystem). This backend implements all the specific traits like `Read`, `Write`, `Lookup`, etc. The `lib.rs` module then treats this backend as a `dyn Vfs<B>`. When an RPC request comes in, the dispatcher calls the appropriate method on this `Vfs` object (e.g., `vfs.read(...)`). The `Vfs` trait guarantees that the backend supports *every* NFSv3 procedure required by the specification.

Inside the system, the following things happen and they use this module:
1. **Type Unification**: The `Vfs` trait aggregates all sub-traits. This allows the `ServerContext` (defined in `lib.rs`) to hold a single `Arc<dyn Vfs>` instead of a collection of trait objects, drastically simplifying the generic parameters of the server.
2. **Protocol Standardization**: The `Error` enum defined here is the single source of truth for error codes. Sub-modules return `vfs::Error`, and the RPC layer maps this enum to the integer status codes defined in the NFSv3 RFC. This ensures that if a new error is added or a code changes, it only needs to be updated in one place.
3. **Data Structure Sharing**: `WccData` and `DirOpArgs` are defined here to be used across multiple sub-modules (e.g., `create`, `remove`, `rename`). This prevents code duplication and ensures that all directory operations report cache consistency data in the exact same format.

The critical aspect of this module is the **blanket implementation** (`impl<T, B> Vfs<B> for T`). This Rust idiom allows any type `T` that implements the specific traits to automatically satisfy the `Vfs` bound. This means backend implementers can focus on implementing individual procedures (e.g., `fn read`) without worrying about manually implementing a massive `Vfs` trait, while the system still enforces that *all* procedures are present before the backend can be used as a `Vfs`.