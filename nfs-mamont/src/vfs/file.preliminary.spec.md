<!-- SPEC_HASH: cc4695a92c62540bb6b398b270542bbe7bd3b34792bcda3889ce8eee5a6fe12b -->
# Module Specification

Module: nfs_mamont::vfs::file
Rust File: src/vfs/file.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **std::io**: Used to provide the `io::Result` type and `io::Error` struct for reporting validation failures (e.g., strings exceeding maximum length) in the constructors of `Name` and `Path`.
- **std::path::PathBuf**: Used as the underlying storage mechanism within the `Path` struct to handle file system path manipulation.
- **num_derive**: Used to derive `FromPrimitive` and `ToPrimitive` for the `Type` enum, enabling conversion between integer representations (received over the network via NFSv3) and the Rust enum variants.
- **crate::vfs**: Used to import `MAX_NAME_LEN` and `MAX_PATH_LEN` constants. These constants define the upper bounds for string lengths in `Name` and `Path`, ensuring compliance with the VFS layer's constraints.
- **crate::consts::nfsv3**: Used to import `NFS3_FHSIZE`. This constant defines the fixed size (64 bytes) of the file handle (`Handle`), ensuring the structure matches the NFSv3 protocol specification (RFC 1813).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define a set of strongly-typed, validated data structures that represent file system entities (handles, names, paths, attributes) used throughout the VFS layer.
- To encapsulate the validation logic for protocol-specific limits (e.g., name length, path length) within the constructors of these types, preventing invalid data from propagating deeper into the system.
- To provide a mapping between the raw data types defined by the NFSv3 protocol (u32, opaque arrays) and semantic Rust types (enums, structs).

Inputs:
- Raw `String` data for `Name` and `Path`.
- Raw byte arrays `[u8; NFS3_FHSIZE]` for `Handle`.
- Integer values for `Type`, `Time`, `Device`, and `Attr` fields.

Outputs:
- Initialized instances of `Handle`, `Name`, `Path`, `Attr`, `Type`, `Time`, `Device`, or `WccAttr`.
- `io::Error` if validation fails during `Name` or `Path` construction.

Steps:
1.  **Handle Construction**: The `Handle` struct wraps a fixed-size byte array. It acts as a unique identifier for a file, corresponding to the NFSv3 file handle. No runtime validation is performed on construction beyond the type-level size guarantee.
2.  **Name Validation**: When `Name::new` is called:
    *   The length of the input string is compared against `MAX_NAME_LEN`.
    *   The string is checked for emptiness.
    *   The string is checked for the presence of the path separator character `/`.
    *   If any check fails, an `io::Error` with `InvalidInput` kind is returned.
    *   If all checks pass, the `Name` wrapper is instantiated.
3.  **Path Validation**: When `Path::new` is called:
    *   The length of the input string is compared against `MAX_PATH_LEN`.
    *   The string is checked for emptiness.
    *   If any check fails, an `io::Error` with `InvalidInput` kind is returned.
    *   If all checks pass, the `Path` wrapper is instantiated, storing the data internally as `PathBuf`.
4.  **Attribute Representation**: The `Attr` struct aggregates various metadata fields (mode, uid, gid, size, timestamps, etc.) into a single structure. It relies on `Type`, `Time`, and `Device` to structure specific sub-fields.
5.  **Type Conversion**: The `Type` enum derives `FromPrimitive` and `ToPrimitive`, allowing the system to convert between the integer discriminants used in the NFS protocol and the semantic file types (Regular, Directory, etc.).

Edge Cases:
- **Name/Path Construction**: Input strings that are exactly at the maximum length limit (`MAX_NAME_LEN` or `MAX_PATH_LEN`) are accepted, while any string exceeding this limit by one byte is rejected.
- **Name Separator**: A `Name` string containing a `/` character is rejected, enforcing that `Name` represents a single component, not a full path.

Complexity:
- Time: O(N) for `Name` and `Path` construction, where N is the length of the input string (due to length checks and character search).
- Space: O(N) for `Name` and `Path`, as they own the underlying string data. O(1) for `Handle`, `Type`, `Time`, `Device`, and `WccAttr`.

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::vfs` (Assumption based on usage)**: Provides the constants `MAX_NAME_LEN` and `MAX_PATH_LEN`. These are critical boundary values used in the validation logic of `Name` and `Path` to enforce protocol constraints.
- **From `crate::consts::nfsv3`**: Provides the constant `NFS3_FHSIZE`. This defines the fixed size of the `Handle` structure, ensuring it matches the wire format requirements of NFSv3.

---

## 4. Data Model

Entities:
- **Handle**: A unique file identifier represented by a fixed-size byte array `[u8; NFS3_FHSIZE]`. It corresponds to the file handle in RFC 1813.
- **Name**: A validated wrapper around a `String` representing a single file or directory name. It guarantees the absence of path separators and adherence to length limits.
- **Path**: A validated wrapper around `PathBuf` representing a file system path. It guarantees adherence to maximum length limits.
- **Type**: An enumeration representing the file type (Regular, Directory, BlockDevice, CharacterDevice, Symlink, Socket, Fifo).
- **Attr**: A structure containing comprehensive file attributes, including type, mode, ownership (uid/gid), size, usage, device ID, filesystem ID, file ID, and timestamps (atime, mtime, ctime).
- **Time**: A structure representing time with seconds and nanoseconds since the Unix epoch.
- **Device**: A structure representing a device ID via major and minor numbers.
- **WccAttr**: A structure containing a subset of attributes (size, mtime, ctime) used for Weak Cache Consistency, representing the state of an object before an operation.

Relations:
- **Composition**: `Attr` contains instances of `Type`, `Device`, and `Time`.
- **Composition**: `WccAttr` contains instances of `Time`.

Global Invariants:
- **Name Invariants**: The inner string of a `Name` instance is never empty, never contains `/`, and its length never exceeds `MAX_NAME_LEN`.
- **Path Invariants**: The inner string of a `Path` instance is never empty, and its length never exceeds `MAX_PATH_LEN`.
- **Handle Invariants**: The `Handle` array is always exactly `NFS3_FHSIZE` bytes long.

## 5. Error Model

Error Types:
- `std::io::Error`

Error Propagation Strategy:
- The module uses `std::io::Result<Self>` for the constructors of `Name` and `Path`. Errors are constructed using `io::Error::new(io::ErrorKind::InvalidInput, message)`.

Recoverability:
- Recoverable. Callers of `Name::new` or `Path::new` must handle the `Result` to decide how to proceed (e.g., rejecting an NFS request).

Panics:
- Allowed: No
- Conditions: The public API does not contain any `panic!` calls or operations that could panic (like index out of bounds) given the provided validation logic.

---

## 6. Traits

List which external traits this module implements:
- **std::fmt::Debug**: Implemented for `Handle`, `Name`, `Path`, `Type`, `Attr`, `Time`, `Device`, `WccAttr`.
- **std::clone::Clone**: Implemented for `Handle`, `Name`, `Path`, `Type`, `Attr`, `Time`, `Device`, `WccAttr`.
- **std::cmp::PartialEq**: Implemented for `Handle`, `Name`, `Path`, `Attr`, `Time`, `WccAttr`.
- **std::cmp::Eq**: Implemented for `Handle`, `Path`.
- **std::hash::Hash**: Implemented for `Handle`, `Path`.
- **num_traits::FromPrimitive**: Implemented for `Type`.
- **num_traits::ToPrimitive**: Implemented for `Type`.
- **std::marker::Copy**: Implemented for `Handle`, `Type`, `Time`, `Device`, `WccAttr`.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the core data types that serve as the currency of information exchange within the Virtual File System (VFS) layer of the `nfs_mamont` NFS server. The system contains a complex implementation of an NFSv3 server that must translate between raw network packets and high-level file system operations. A typical usage scenario of the system involves the server receiving an NFS request (e.g., `CREATE` or `LOOKUP`) containing a filename as a raw string. The system uses the `Name` struct from this module to validate that the filename does not contain path separators (preventing directory traversal attacks or protocol violations) and does not exceed the length limit defined by the VFS. Similarly, when the server needs to return file metadata to the client, it constructs an `Attr` struct, which aggregates type information, permissions, and timestamps into a format that matches the NFSv3 `fattr3` specification.

Inside the system, the following things happen and they use this module: The `Vfs` trait (defined in the parent `vfs` module) relies on `Handle` to uniquely identify files without exposing internal inode numbers or paths to the client. The `WccAttr` struct is used to optimize network traffic by allowing the server to send only the attributes that have changed (or the pre-operation attributes) rather than the full attribute set, which is crucial for maintaining cache consistency on the client side with minimal overhead. Without this module, the VFS implementation would lack a standardized, validated set of types, leading to potential inconsistencies between the data sent over the wire and the data processed by the storage backend, and increasing the risk of security vulnerabilities arising from unvalidated input.