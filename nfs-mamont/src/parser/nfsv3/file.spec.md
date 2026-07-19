<!-- SPEC_HASH: 2fb7514d12f49246df77b022a3c2c23091e0dcf4fe1d12707fa79402634ac082 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::file
Rust File: src/parser/nfsv3/file.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in all parsing functions. It allows the parser to read bytes from various sources (network streams, buffers, etc.) in a generic way.
- **`crate::parser::primitive`**: Used to perform low-level XDR (External Data Representation) parsing. Functions like `u32`, `u64`, `array`, and `string_max_size` are called to extract primitive data types from the stream, handling endianness and padding automatically.
- **`crate::vfs::file`**: Used to define the target types that this module constructs. The functions return instances of `file::Handle`, `file::Type`, `file::Attr`, `file::Name`, etc., effectively deserializing the wire format into the VFS domain model.
- **`crate::vfs`**: Used to import constants `MAX_NAME_LEN` and `MAX_PATH_LEN`. These are passed to `string_max_size` to enforce protocol-level limits on string lengths during parsing.
- **`crate::consts::nfsv3`**: Used to import `NFS3_FHSIZE`. This constant is used in the `handle` function to validate that the file handle size read from the wire matches the NFSv3 specification (64 bytes).
- **`crate::parser`**: Used to import the `Error` enum and `Result` type alias. This allows the module to report parsing failures (e.g., `IO`, `EnumDiscMismatch`, `MaxElemLimit`) consistently with the rest of the parser subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the specific file system structures defined by the NFSv3 protocol (RFC 1813) from a raw byte stream into the high-level, validated types used by the `nfs_mamont` VFS layer.
- To map raw integer discriminants to semantic Rust enums (e.g., file types).
- To bridge the gap between the loose validation of the wire format (handled by `primitive`) and the strict validation of the VFS types (handled by `vfs::file` constructors).

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`, representing the stream of bytes being parsed.

Outputs:
- `Result<T>`: A `Result` containing the parsed VFS type (e.g., `file::Handle`, `file::Attr`) or a `parser::Error`.

Steps:
1. **Primitive Extraction**: For every field in a structure (e.g., `mode` in `Attr`), the corresponding primitive parser from `crate::parser::primitive` is called (e.g., `u32(src)`).
2. **Enum Mapping**: For the `r#type` function, a `u32` is read and matched against specific integer values defined by the NFSv3 protocol to construct the `file::Type` enum (e.g., `1` -> `Regular`, `2` -> `Directory`).
3. **Structure Aggregation**: For complex structures like `file::Attr`, `file::Time`, or `file::Device`, the individual fields are parsed sequentially and aggregated into the target struct.
4. **Handle Validation**: In the `handle` function, the length of the file handle is read first. If it does not match `NFS3_FHSIZE`, `Error::BadFileHandle` is returned immediately. Otherwise, the byte array is read and wrapped.
5. **String Validation**: In `file_name` and `file_path`, a string is read with a maximum length constraint. The resulting `String` is then passed to the `new` constructor of `file::Name` or `file::Path`.
6. **Error Mapping**: The `map_validation_error` helper intercepts `io::Error` from the VFS constructors. If the error indicates the input was "too long", it is converted to `Error::MaxElemLimit`. Other IO errors are wrapped in `Error::IO`.

Edge Cases:
- **Invalid File Handle Size**: If the size prefix in the stream for a file handle is not exactly `NFS3_FHSIZE`, the function returns `Error::BadFileHandle` without reading the body.
- **Unknown File Type**: If the integer discriminant for `file::Type` is not within the range 1-7, the function returns `Error::EnumDiscMismatch`.
- **String Validation Failure**: If a parsed string exceeds the VFS limits or contains invalid characters (e.g., `/` in a `Name`), the `io::Error` from the constructor is caught and mapped to a parser error.

Complexity:
- Time: O(N) for variable-length fields (strings, arrays), where N is the length of the data. O(1) for fixed-size structures (e.g., `Time`, `Device`, `Type`).
- Space: O(N) for allocated strings and arrays. O(1) for stack-allocated structures.

Determinism:
- Deterministic. Given the same input byte stream, the functions will always produce the same output or error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
 - **XDR Compliance**: The module relies on `primitive` to handle the intricacies of XDR, such as big-endian byte order for integers (`u32`, `u64`) and 4-byte alignment for opaque data (`array`).
 - **Length Limits**: The `string_max_size` function is used to prevent allocation of excessively large strings from the network, enforcing `MAX_NAME_LEN` and `MAX_PATH_LEN` at the read level.

- **From `nfs_mamont::vfs::file`**:
 - **Type Safety**: The module constructs the strongly-typed structs defined here (e.g., `file::Attr`, `file::Handle`).
 - **Validation Logic**: The constructors `Name::new` and `Path::new` are used to enforce semantic rules (like "no slashes in filenames") that are purely VFS-level concerns, not wire-format concerns.

- **From `nfs_mamont::vfs`**:
 - **Constants**: The module uses `MAX_NAME_LEN` and `MAX_PATH_LEN` to define the boundaries of acceptable data during parsing, ensuring the server does not accept paths or names that the backend filesystem cannot handle.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a transformer, converting byte streams into the entities defined in `nfs_mamont::vfs::file`.

Relations:
- **Transformation**: The functions in this module act as adapters between the `Read` stream and the `vfs::file` structs.

Global Invariants:
- **NFSv3 Compliance**: The parsed structures must conform to the field order and types defined in RFC 1813. For example, `file::Attr` requires `type` to be parsed before `mode`.
- **Handle Size**: The `file::Handle` produced by the `handle` function is guaranteed to be exactly `NFS3_FHSIZE` bytes long.

## 5. Error Model

Error Types:
- **`parser::Error`**: The primary error type used throughout the module.
 - `Error::BadFileHandle`: Returned when the file handle size in the stream is incorrect.
 - `Error::EnumDiscMismatch`: Returned when an unknown file type integer is encountered.
 - `Error::MaxElemLimit`: Returned when a parsed string is too long for the VFS.
 - `Error::IO(io::Error)`: Returned for underlying read errors or other validation failures from the VFS layer.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used extensively to propagate errors from `primitive` parsers.
- **Mapping**: A custom helper `map_validation_error` converts `std::io::Error` (from VFS constructors) into `parser::Error` variants, specifically distinguishing "too long" errors as `MaxElemLimit`.

Recoverability:
- **Unrecoverable for the current operation**: If a parse fails, the stream cursor may be in an undefined state relative to the higher-level message structure, typically necessitating the abandonment of the current RPC request.

Panics:
- Allowed: No
- Conditions: The code uses `map_err` and explicit checks to handle errors gracefully. No `unwrap` or `expect` calls are present in the parsing logic.

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This module is used in order to **translate the binary representation of file system metadata from the NFSv3 protocol into the internal, type-safe objects used by the server's Virtual File System (VFS)**. The system contains a complex parser hierarchy where `primitive` handles the raw bytes and `vfs::file` defines the semantic objects. This module sits in the middle, implementing the specific logic required to assemble those objects according to the NFSv3 specification (RFC 1813).

A typical usage scenario of the system involves the server receiving an NFSv3 `LOOKUP` reply. The reply contains a file handle and a set of file attributes. The RPC dispatcher calls the `handle` function in this module to read the 64-byte handle and the `attr` function to read the metadata (mode, uid, size, timestamps, etc.). These parsed objects are then passed to the VFS layer to update its state or construct a response to the client.

Inside the system, the following things happen and they use this module:
- **Protocol Enforcement**: The `handle` function strictly enforces the `NFS3_FHSIZE` constant, rejecting any handle that doesn't match the expected 64-byte length, which protects the server from malformed packets.
- **Semantic Mapping**: The `r#type` function maps raw integers (1-7) to the `file::Type` enum (Regular, Directory, etc.), ensuring that the rest of the server code deals with meaningful types rather than magic numbers.
- **Validation Bridging**: The `file_name` and `file_path` functions use the VFS constructors to validate strings immediately after parsing them. This ensures that data entering the system is checked against both protocol limits (via `string_max_size`) and filesystem limits (via `Name::new`), providing a defense-in-depth strategy against invalid input.

Without this module, the parsing logic for NFSv3 file structures would be scattered throughout the RPC handlers or mixed with low-level byte manipulation, leading to code duplication and a higher risk of protocol violations. This module centralizes the "deserialization" logic for file metadata, ensuring a clean separation between the network protocol format and the internal file system model.