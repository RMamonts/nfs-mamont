<!-- SPEC_HASH: a03e0ba0170702a5d26ac51421abd42f926614abd563ee39bb8826ce6450f05f -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::fs_info
Rust File: src/parser/nfsv3/fs_info.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in the `args` function. It allows the parser to read bytes from a generic stream (e.g., TCP stream or memory buffer).
- **`crate::parser::nfsv3::file`**: Used to parse the file handle from the input stream. The `handle` function is called to extract the `file::Handle` object, which represents the root of the file system being queried.
- **`crate::vfs::fs_info`**: Used to provide the target structure `fs_info::Args` that the `args` function constructs. This structure defines the expected input for the VFS layer's `FSINFO` operation.
- **`crate::parser`**: Used to import the `Result` type alias, which standardizes error handling across the parser subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `FSINFO` procedure from a byte stream into a structured Rust type compatible with the VFS layer.
- To act as a specific adapter that maps the wire format (a single file handle) to the domain model (`fs_info::Args`).

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`. This represents the source of the XDR-encoded data.

Outputs:
- `Result<fs_info::Args>`: A Result containing the parsed arguments on success, or a `parser::Error` on failure (propagated from the `file` module).

Steps:
1. **Handle Parsing**: The function calls `file::handle(src)` to read the file handle from the stream. This operation consumes the bytes representing the handle length and the handle data itself from `src`.
2. **Argument Construction**: The parsed `file::Handle` is wrapped in the `fs_info::Args` struct as the `root` field.
3. **Return**: The `fs_info::Args` struct is returned wrapped in `Ok`.

Edge Cases:
- **Stream Exhaustion**: If `src` ends before the full file handle can be read, `file::handle` will return an `IO` error, which propagates through `args`.
- **Invalid Handle**: If the handle data in the stream is malformed (e.g., incorrect length according to the `file` module's validation), an error is returned immediately.

Complexity:
- **Time**: O(N), where N is the size of the file handle (typically fixed size, e.g., 64 bytes). The complexity is dominated by the read operation.
- **Space**: O(N), to store the parsed `file::Handle` within the `Args` struct.

Determinism:
- **Deterministic**: Given the same input byte stream, the function will always produce the same `fs_info::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle` function**: This is the core parsing primitive used by this module. It handles the low-level XDR decoding (reading the length prefix and the byte array) and validates the handle size against `NFS3_FHSIZE`. The `fs_info` module delegates all parsing logic to this function.
 - **Validation**: The `fs_info` module relies on `file::handle` to enforce protocol constraints, such as rejecting handles that do not match the expected size.

- **From `nfs_mamont::vfs::fs_info`**:
 - **`Args` struct**: This module serves as the constructor for this struct. The `fs_info::Args` struct is the input type required by the VFS trait for the `FSINFO` operation. By populating the `root` field, this module prepares the data necessary for the VFS to identify which file system is being queried.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a factory for `fs_info::Args`.

Relations:
- **Transformation**: The `args` function transforms a byte stream segment into a `fs_info::Args` instance.
- **Composition**: `fs_info::Args` contains a `file::Handle`, which is parsed from the stream.

Global Invariants:
- The `root` field of the returned `fs_info::Args` is guaranteed to be a valid `file::Handle` as defined by the `file` module (e.g., correct length, valid bytes).

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type returned via the `Result` alias. Specific errors are those returned by `file::handle`, such as `IO` (if reading fails) or `BadFileHandle` (if validation fails).

Error Propagation Strategy:
- **Direct Propagation**: The module uses the `?` operator to immediately return any error encountered during the `file::handle(src)` call. It does not perform any custom error mapping or wrapping.

Recoverability:
- **Unrecoverable for the current message**: If parsing fails, the stream cursor is advanced partially (or indeterministically), and the current RPC request cannot be processed further. The caller (RPC dispatcher) must discard the request.

Panics:
- **Allowed**: No.
- **Conditions**: The code consists of a single call and a struct construction; there are no `unwrap`, `expect`, or index operations that could panic.

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This module is used in order to **parse the arguments for the NFSv3 `FSINFO` procedure**, which allows clients to query static and dynamic parameters of a file system (such as maximum transfer sizes, preferred I/O sizes, and supported properties). The system contains a parser hierarchy where high-level procedure parsers (like this one) delegate the parsing of specific data types (like file handles) to utility modules (like `file`).

A typical usage scenario of the system involves the RPC dispatcher receiving a request for the `FSINFO` procedure. The dispatcher identifies the procedure number and invokes the `args` function in this module, passing the network stream. The function reads the file handle (identifying the target file system) from the stream and wraps it in `fs_info::Args`. This structured argument is then passed to the VFS layer, which uses the handle to look up the specific file system instance and retrieve its capabilities to send back to the client.

Inside the system, the following things happen and they use this module:
- **Delegation of Parsing Logic**: The module delegates the complex task of reading and validating the file handle to the `file` module. This ensures that handle parsing logic (which is shared across many procedures like `LOOKUP`, `READ`, etc.) is not duplicated here.
- **VFS Integration**: By constructing `fs_info::Args`, this module bridges the gap between the network protocol layer (bytes) and the service layer (VFS traits). The VFS layer expects a strongly-typed `Args` struct, and this module is responsible for delivering it.

Without this module, the RPC dispatcher would lack a dedicated function to deserialize `FSINFO` arguments, forcing it to either inline the parsing logic (leading to code duplication) or rely on a generic but less type-safe mechanism. This module encapsulates the specific wire-format requirements of the `FSINFO` procedure arguments.