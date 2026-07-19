<!-- SPEC_HASH: 0903dd2f0526eab4cb2f0a6be5403ae309bd1ea4d5c7b3041dc14293344210bc -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::access
Rust File: src/parser/nfsv3/access.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in the `args` function. It allows the parser to read bytes from a generic stream (e.g., network socket or buffer).
- **`crate::parser::nfsv3::file`**: Used to parse the file handle component of the `ACCESS` arguments. Specifically, the `handle` function is called to read and validate the file identifier from the stream.
- **`crate::parser::primitive::u32`**: Used to parse the raw 32-bit unsigned integer representing the access mask from the stream.
- **`crate::vfs::access`**: Used to define the target structure `access::Args` that the parser returns. It also provides the `access::Mask` type, specifically the `from_wire` method, to convert the raw integer into a sanitized bitmask.
- **`crate::parser::Result`**: Used as the return type for the `args` function, allowing the propagation of parsing errors (e.g., IO errors, invalid data).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `ACCESS` procedure from an XDR (External Data Representation) byte stream into the high-level `vfs::access::Args` structure used by the server's VFS layer.
- To ensure that the raw access mask read from the network is sanitized and converted into the type-safe `Mask` wrapper before being passed to the application logic.

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`, representing the stream of bytes containing the encoded arguments.

Outputs:
- `Result<access::Args>`: A `Result` containing the parsed arguments (`access::Args`) or a `parser::Error` if the stream cannot be read or contains invalid data.

Steps:
1. **File Handle Parsing**: The function calls `file::handle(src)` to read the file handle from the stream. This operation consumes the bytes corresponding to the file handle and validates its size according to NFSv3 standards.
2. **Mask Parsing**: The function calls `primitive::u32(src)` to read the next 4 bytes as a big-endian unsigned integer. This integer represents the raw access permissions requested by the client.
3. **Mask Sanitization**: The raw `u32` value is passed to `access::Mask::from_wire`. This constructor applies a bitmask to ignore any undefined or reserved bits, ensuring the resulting `Mask` object only contains valid protocol flags.
4. **Argument Construction**: The parsed file handle and the sanitized mask are aggregated into an `access::Args` struct, which is then wrapped in `Ok` and returned.

Edge Cases:
- **Stream Exhaustion**: If the stream ends before the file handle or the mask can be fully read, the underlying `read_exact` calls in `file::handle` or `primitive::u32` will return an `IO` error, which propagates through the `?` operator.
- **Invalid Mask Bits**: If the client sets bits in the mask that are not defined in the NFSv3 specification, `Mask::from_wire` silently drops them. This is handled by the dependency and does not cause an error in this module.

Complexity:
- Time: O(1). The function reads a fixed-size file handle (64 bytes) and a 4-byte integer.
- Space: O(1). The function allocates only the stack space required for the `Args` struct and the intermediate values.

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same `access::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle`**: This function is critical for reading the file identifier. It encapsulates the logic for reading the length prefix, validating it against `NFS3_FHSIZE`, and reading the byte array. This module relies on it to provide a valid `file::Handle` for the `Args` struct.

- **From `nfs_mamont::parser::primitive`**:
 - **`u32`**: This function handles the low-level extraction of a 32-bit integer in Big-Endian format. This module uses it to obtain the raw access mask value from the wire.

- **From `nfs_mamont::vfs::access`**:
 - **`Mask::from_wire`**: This mechanism is essential for protocol safety. It ensures that the raw integer read from the network is converted into a `Mask` instance that strictly adheres to the defined access rights (READ, LOOKUP, MODIFY, etc.), stripping any undefined bits.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a constructor for the `access::Args` entity defined in `nfs_mamont::vfs::access`.

Relations:
- **Transformation**: The `args` function transforms a byte stream (`src`) into an `access::Args` struct.

Global Invariants:
- **Field Order**: The byte stream must contain the file handle followed immediately by the 32-bit access mask. This order is mandated by the NFSv3 XDR definition for the `ACCESS` procedure arguments.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type returned via the `Result` alias. This includes variants like `IO` (if reading fails) or `BadFileHandle` (if the handle size is invalid, propagated from `file::handle`).

Error Propagation Strategy:
- **Propagation**: The module uses the `?` operator to propagate errors returned by `file::handle` and `primitive::u32`. It does not introduce new error types or perform custom error mapping.

Recoverability:
- **Unrecoverable for the Message**: If this function returns an `Err`, the `ACCESS` procedure arguments cannot be reconstructed. The RPC layer will typically discard the request and send an error response to the client (e.g., `GARBAGE_ARGS`).

Panics:
- Allowed: No.
- Conditions: The code consists solely of function calls and struct construction. There are no `unwrap`, `expect`, or indexing operations that could panic.

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **deserialize the specific arguments for the NFSv3 `ACCESS` procedure**, translating the raw network byte stream into a structured, type-safe representation that the server's Virtual File System (VFS) can act upon. The system contains a layered parsing architecture where low-level modules handle byte alignment and primitive types, mid-level modules handle file system specific structures (like file handles), and high-level modules like this one assemble the final procedure arguments.

A typical usage scenario of the system involves the RPC dispatcher receiving a request message identified as an NFSv3 `ACCESS` call. The dispatcher invokes this `args` function, passing the remaining bytes of the message. The function reads the file handle to identify the target object and reads the access mask to determine what permissions the client is querying. The resulting `access::Args` struct is then passed to the VFS implementation to perform the actual access check.

Inside the system, the following things happen and they use this module:
- **Protocol Enforcement**: By delegating the file handle parsing to `file::handle`, this module ensures that only valid, correctly sized handles are accepted, rejecting malformed requests early.
- **Data Sanitization**: By using `Mask::from_wire`, the module ensures that the access mask entering the system is clean. Even if the client sends a mask with reserved bits set, this module (via its dependency) normalizes the data, preventing undefined behavior in the VFS layer.
- **Abstraction**: This module hides the details of the XDR format (field order, integer endianness) from the VFS logic. The VFS sees only `Args { file, mask }`, allowing the storage backend to remain agnostic to the network protocol specifics.

Without this module, the RPC layer would need to manually assemble the `ACCESS` arguments by calling primitive parsers directly, scattering the protocol definition logic and increasing the risk of mismatches between the wire format and the VFS interface. This module centralizes the "decoding" logic for the `ACCESS` procedure, ensuring a clean separation of concerns.