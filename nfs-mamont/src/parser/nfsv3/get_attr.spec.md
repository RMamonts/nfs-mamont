<!-- SPEC_HASH: f099458ab983335613c76313712778cea1f35ced586114576f96f058816b6a6d -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::get_attr
Rust File: src/parser/nfsv3/get_attr.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the generic source trait for the `src` parameter in the `args` function. It allows the parser to consume bytes from any stream (e.g., TCP socket, memory buffer) that implements the standard read interface.
- **`crate::parser::nfsv3::file`**: Used to parse the file handle from the input stream. The `file::handle` function is called to deserialize the opaque file handle bytes into a `file::Handle` structure, validating the length and content according to NFSv3 specifications.
- **`crate::vfs::get_attr`**: Used to define the return type of the `args` function. The `get_attr::Args` struct is the target data structure that holds the parsed file handle, which is eventually passed to the VFS layer's `GetAttr` trait.
- **`crate::parser`**: Used to import the `Result` type alias. This ensures that the function returns the canonical error type (`parser::Error`) used throughout the parsing subsystem, facilitating consistent error handling (e.g., I/O errors, invalid data).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `GETATTR` procedure from a byte stream into a structured Rust type (`get_attr::Args`).
- To act as a bridge between the raw network protocol format (handled by `file`) and the high-level VFS interface (defined in `vfs::get_attr`).

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`. This represents the stream of bytes containing the encoded arguments.

Outputs:
- `Result<get_attr::Args>`: A Result containing the parsed arguments on success, or a `parser::Error` on failure.

Steps:
1. The function `args` is invoked with a byte stream `src`.
2. It calls `file::handle(src)` to read and parse the file handle from the stream. This operation consumes the bytes representing the handle length and the handle data itself.
3. If `file::handle` returns successfully, the resulting `file::Handle` is used to initialize the `file` field of a `get_attr::Args` struct.
4. The `get_attr::Args` struct is wrapped in `Ok` and returned.
5. If `file::handle` returns an `Err` (e.g., due to an invalid handle length or I/O failure), that error is propagated immediately via the `?` operator.

Edge Cases:
- **Stream Exhaustion**: If the stream ends before the file handle can be fully read, `file::handle` will return an I/O error, which propagates out of `args`.
- **Invalid Handle Length**: If the stream indicates a handle length that does not match the expected `NFS3_FHSIZE`, `file::handle` returns `Error::BadFileHandle`, causing `args` to fail.

Complexity:
- Time: O(1). The file handle has a fixed maximum size defined by the protocol, so the read operation is bounded by a constant.
- Space: O(1). The `get_attr::Args` struct contains a single fixed-size `file::Handle`.

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same `get_attr::Args` struct or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle` function**: This is the primary parsing primitive used. It encapsulates the logic for reading the XDR-encoded file handle, including reading the length prefix and the byte array, and validating that the length matches `NFS3_FHSIZE`. This module relies on it to convert raw bytes into the `file::Handle` type required by the VFS.

- **From `nfs_mamont::vfs::get_attr`**:
 - **`Args` struct**: This module constructs this specific struct. The `Args` struct acts as the data carrier that moves the parsed handle from the network layer (parser) to the execution layer (VFS). The parser ensures that the `file` field of `Args` is populated correctly before returning.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It constructs an instance of `vfs::get_attr::Args`.

Relations:
- **Construction**: The `args` function constructs a `vfs::get_attr::Args` entity.
- **Composition**: The `vfs::get_attr::Args` entity contains a `vfs::file::Handle` entity, which is parsed by the `file` module.

Global Invariants:
- The `file` field in the returned `Args` must be a valid `file::Handle` as defined by the `nfs_mamont::vfs::file` module (specifically, it must be `NFS3_FHSIZE` bytes long).

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type returned via the `Result` alias. This includes errors propagated from `file::handle`, such as:
 - `Error::IO`: Underlying read failure.
 - `Error::BadFileHandle`: The handle length in the stream was invalid.

Error Propagation Strategy:
- **Direct Propagation**: The module uses the `?` operator on the result of `file::handle(src)`. It does not introduce new error types or modify existing errors; it simply passes them up to the caller.

Recoverability:
- **Unrecoverable for the Message**: If this function returns an `Err`, the arguments for the `GETATTR` procedure cannot be recovered. The RPC request is considered malformed and must be rejected by the higher-level dispatcher.

Panics:
- Allowed: No
- Conditions: The code consists of a single call to a fallible function followed by a struct construction. There are no `unwrap`, `expect`, or index operations that could panic.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **parse the arguments for the NFSv3 `GETATTR` procedure** from the network byte stream. The system contains a complex parser hierarchy where the RPC dispatcher identifies the procedure type and delegates the deserialization of arguments to specific modules. This module is the specific handler for `GETATTR`.

A typical usage scenario of the system involves the RPC dispatcher receiving a request message. It determines that the procedure is `GETATTR` (procedure number 1 in NFSv3). The dispatcher then calls the `args` function in this module, passing the remaining bytes of the request. The function reads the file handle (which identifies the target object) and returns it wrapped in `get_attr::Args`. The dispatcher then passes this `Args` struct to the VFS implementation, specifically to the `get_attr` method of the `GetAttr` trait, to retrieve the file's attributes.

Inside the system, the following things happen and they use this module:
- **Protocol Decoding**: The system relies on this module to interpret the specific wire format for `GETATTR` arguments. While the `file` module knows how to read a generic file handle, this module provides the context that "this handle is for a GETATTR operation."
- **Type Safety**: By returning `vfs::get_attr::Args`, the module ensures that the rest of the server receives a strongly-typed structure rather than raw bytes, preventing accidental misuse of the data (e.g., treating a file handle as a directory entry name).

Without this module, the RPC dispatcher would need to contain inline logic to parse `GETATTR` arguments, breaking the separation of concerns between protocol transport logic and procedure-specific logic. This module encapsulates the knowledge of how `GETATTR` arguments are structured on the wire.