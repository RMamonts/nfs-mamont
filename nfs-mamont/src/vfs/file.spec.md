<!-- SPEC_HASH: cc4695a92c62540bb6b398b270542bbe7bd3b34792bcda3889ce8eee5a6fe12b -->
# Module Specification

Module: nfs_mamont::vfs::file
Rust File: src/vfs/file.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**: Used to provide the `io::Result` type and `io::Error` struct. The constructors of `Name` and `Path` return `io::Result` to signal validation failures (e.g., strings exceeding maximum length) using the `InvalidInput` error kind.
- **`std::path::PathBuf`**: Used as the underlying storage for the `Path` struct. It provides the necessary path manipulation capabilities, though the `Path` wrapper restricts access to ensure validation invariants are maintained.
- **`num_derive`**: Used to derive `FromPrimitive` and `ToPrimitive` traits for the `Type` enum. This allows conversion between the enum variants and their integer discriminants, which is required for mapping file types to and from the NFSv3 wire protocol.
- **`crate::vfs`**: Used to import the constants `MAX_NAME_LEN` and `MAX_PATH_LEN`. These constants define the upper bounds for the length of file names and paths, respectively, which are enforced by the `Name` and `Path` constructors.
- **`crate::consts::nfsv3`**: Used to import the constant `NFS3_FHSIZE`. This constant defines the fixed size (in bytes) of the file handle, which determines the size of the array stored within the `Handle` struct.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the core data structures representing file system entities (handles, names, paths, attributes) used throughout the NFSv3 implementation.
- To enforce protocol-level constraints (e.g., maximum name length, absence of separators in names) at the boundary of the VFS layer using "Newtype" wrappers with validation logic.
- To provide a standard mapping between Rust types and the NFSv3 protocol data structures defined in RFC 1813 (e.g., `fattr3`, `nfs_fh3`).

Inputs:
- **`Name::new`**: A `String` representing a file name.
- **`Path::new`**: A `String` representing a file path.
- **`Handle`**: A raw byte array `[u8; NFS3_FHSIZE]` (though the struct wraps this directly, it is the input for the system).

Outputs:
- **`Name`**: A validated wrapper containing the name string.
- **`Path`**: A validated wrapper containing the `PathBuf`.
- **`Handle`**: A struct wrapping the fixed-size file handle byte array.
- **`Attr`**: A struct containing comprehensive file metadata (type, mode, size, IDs, timestamps).
- **`Type`**: An enum representing the file type (Regular, Directory, etc.).
- **`WccAttr`**: A struct containing a subset of attributes (size, mtime, ctime) used for Weak Cache Consistency.

Steps:
1. **Validation (`Name::new`)**:
   - Checks if the input string length is greater than `MAX_NAME_LEN`. If so, returns `Err(io::ErrorKind::InvalidInput)`.
   - Checks if the input string is empty. If so, returns `Err`.
   - Checks if the input string contains the path separator `/`. If so, returns `Err`.
   - If all checks pass, wraps the string in the `Name` struct and returns `Ok`.
2. **Validation (`Path::new`)**:
   - Checks if the input string length is greater than `MAX_PATH_LEN`. If so, returns `Err`.
   - Checks if the input string is empty. If so, returns `Err`.
   - If valid, converts the string to a `PathBuf`, wraps it in the `Path` struct, and returns `Ok`.
3. **Data Representation**:
   - `Handle` stores the file handle as a fixed-size array `[u8; NFS3_FHSIZE]`.
   - `Attr` aggregates various metadata fields (mode, uid, gid, size, etc.) into a single structure, mirroring the `fattr3` structure in the NFSv3 protocol.
   - `Type` derives `FromPrimitive` and `ToPrimitive` to support serialization/deserialization of file types as integers.

Edge Cases:
- **Empty Inputs**: Both `Name::new` and `Path::new` explicitly reject empty strings, returning an `InvalidInput` error.
- **Path Separators in Names**: `Name::new` specifically rejects strings containing `/`, ensuring that a `Name` always represents a single component, not a path.
- **Length Limits**: Inputs exceeding `MAX_NAME_LEN` or `MAX_PATH_LEN` are rejected immediately upon construction.

Complexity:
- **Time**:
  - `Name::new`: O(N) where N is the length of the input string (due to `len()` check and `contains()` scan).
  - `Path::new`: O(N) where N is the length of the input string (due to `len()` check and `PathBuf` allocation).
- **Space**:
  - `Name` and `Path` allocate memory proportional to the length of the input string.
  - `Handle`, `Attr`, `Time`, `Device`, `WccAttr` have fixed stack sizes.

Determinism:
- **Deterministic**: All validation logic and data transformations are deterministic and depend solely on the input values.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
  - **Constants**: The module relies on `MAX_NAME_LEN` and `MAX_PATH_LEN` from the parent `vfs` module to define the validation thresholds for `Name` and `Path`. This ensures that the VFS layer enforces the same limits as the broader system configuration.
  - **Usage in Parent**: The parent `vfs` module uses the types defined here (`Handle`, `Name`, `Attr`, `WccAttr`) to construct higher-level structures like `DirOpArgs` and `WccData`. This module provides the fundamental building blocks for VFS operations.

- **From `nfs_mamont::consts::nfsv3`**:
  - **Protocol Constants**: The module relies on `NFS3_FHSIZE` to define the size of the `Handle` struct. This ensures that the file handle representation strictly adheres to the 64-byte size specified in the NFSv3 protocol (RFC 1813).

---

## 4. Data Model

Entities:
- **`Handle`**: A unique identifier for a file or directory.
  - Fields: `0: [u8; NFS3_FHSIZE]`.
- **`Name`**: A validated string representing a single file system entry name.
  - Fields: `0: String`.
  - Invariants: `0.len() <= MAX_NAME_LEN`, `0.len() > 0`, `0` does not contain `/`.
- **`Path`**: A validated string representing a file system path.
  - Fields: `0: PathBuf`.
  - Invariants: `0.as_os_str().len() <= MAX_PATH_LEN`, `0` is not empty.
- **`Type`**: An enumeration of file types.
  - Variants: `Regular`, `Directory`, `BlockDevice`, `CharacterDevice`, `Symlink`, `Socket`, `Fifo`.
- **`Attr`**: A collection of file attributes (metadata).
  - Fields: `file_type`, `mode`, `nlink`, `uid`, `gid`, `size`, `used`, `device`, `fs_id`, `file_id`, `atime`, `mtime`, `ctime`.
- **`Time`**: A timestamp representing seconds and nanoseconds since the Unix epoch.
  - Fields: `seconds: u32`, `nanos: u32`.
- **`Device`**: Identifies a device file.
  - Fields: `major: u32`, `minor: u32`.
- **`WccAttr`**: A subset of attributes used for Weak Cache Consistency.
  - Fields: `size`, `mtime`, `ctime`.

Relations:
- **Composition**: `Attr` contains instances of `Type`, `Device`, and `Time`.
- **Subset**: `WccAttr` contains a subset of the fields present in `Attr` (specifically `size`, `mtime`, `ctime`).

Global Invariants:
- **Handle Size**: The `Handle` array is always exactly `NFS3_FHSIZE` bytes long.
- **Name Purity**: A `Name` instance never represents a path (no separators) and is never empty.
- **Path Validity**: A `Path` instance is never empty and respects the system's maximum path length.

## 5. Error Model

Error Types:
- **`std::io::Error`**: The only error type produced by this module, specifically within the `new` constructors of `Name` and `Path`.

Error Propagation Strategy:
- **Constructor Validation**: The module uses the "Fallible Constructor" pattern. `Name::new` and `Path::new` return `io::Result<Self>`. If validation fails, they return `Err(io::Error::new(io::ErrorKind::InvalidInput, "reason"))`.

Recoverability:
- **Recoverable**: Callers can handle the `Err` result, typically by mapping it to an appropriate NFSv3 status code (e.g., `NFS3ERR_NAMETOOLONG` or `NFS3ERR_INVAL`) to be returned to the client.

Panics:
- **Allowed**: No.
- **Conditions**: The module performs explicit checks and returns `Result` types rather than panicking on invalid input.

---

## 6. Traits

List which external traits this module implements:
- **`Debug`**: Implemented for `Handle`, `Name`, `Path`, `Attr`, `Time`, `WccAttr`, `Type`, `Device`.
- **`Clone`**: Implemented for `Handle`, `Name`, `Path`, `Attr`, `Time`, `WccAttr`, `Type`, `Device`.
- **`PartialEq`**: Implemented for `Handle`, `Name`, `Path`, `Attr`, `Time`, `WccAttr`.
- **`Eq`**: Implemented for `Handle`, `Path`, `Attr`, `Type`.
- **`Hash`**: Implemented for `Handle`, `Path`.
- **`FromPrimitive`**: Implemented for `Type` (via `num_derive`).
- **`ToPrimitive`**: Implemented for `Type` (via `num_derive`).

List which traits this module defines:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than this module.

This module is used in order to **define the fundamental domain types and enforce protocol invariants** for the Virtual File System (VFS) layer of the `nfs_mamont` NFSv3 server. The system contains a complex architecture where raw network data must be translated into safe, structured Rust objects that can be manipulated by the storage backend and the RPC logic. This module serves as the "vocabulary" for the entire VFS, ensuring that all operations speak the same language regarding file identities, names, and attributes.

A typical usage scenario of the system involves the RPC layer receiving a `LOOKUP` request from a client containing a filename. The RPC layer parses this into a raw `String`. Before passing this to the storage backend, it must be converted into a `Name` using `Name::new`. This constructor acts as a firewall: if the string is too long or contains invalid characters (like `/`), the conversion fails immediately. This prevents the backend from ever having to handle invalid inputs, adhering to the "Parse, don't validate" principle. Once the backend performs the lookup, it returns a `Handle` (the unique ID) and an `Attr` (the metadata). These types are defined in this module and are guaranteed to conform to the NFSv3 specification (e.g., `Handle` is exactly 64 bytes). The RPC layer then serializes these structures directly back to the client.

Inside the system, the following things happen and they use this module:
1.  **Type Safety**: The `vfs` module uses `Handle` and `Name` to define `DirOpArgs`. By using these types, the compiler guarantees that a directory operation argument always contains a valid directory handle and a valid entry name, eliminating a whole class of runtime errors.
2.  **Cache Consistency**: The `vfs` module uses `Attr` and `WccAttr` to define `WccData`. The `Attr` struct provides the full post-operation state, while `WccAttr` provides the minimal pre-operation state required for the Weak Cache Consistency mechanism defined in NFSv3. This allows the server to efficiently inform clients about changes to file metadata.
3.  **Protocol Compliance**: The `Type` enum, deriving `FromPrimitive` and `ToPrimitive`, ensures that the integer values sent over the wire for file types (e.g., 1 for Regular, 2 for Directory) are correctly mapped to Rust types, preventing mismatches between the wire format and the internal logic.

Without this module, the VFS would rely on loose types like `String` and `Vec<u8>`, scattering validation logic (length checks, separator checks) throughout the codebase. This would lead to code duplication, potential security vulnerabilities (e.g., path traversal attacks if `/` checks are missed), and difficulty in ensuring compliance with the NFSv3 RFC. This module centralizes these definitions, providing a single source of truth for what a "file" looks like in this system.