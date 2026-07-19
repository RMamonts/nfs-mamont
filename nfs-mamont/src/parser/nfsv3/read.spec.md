<!-- SPEC_HASH: 7c32bb43b46c0753ced00c1242f39aabed5aef2308d8495b0d75374169405704 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::read
Rust File: src/parser/nfsv3/read.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in the `args` function. It allows the parser to read bytes from a generic stream (e.g., TCP stream or buffer).
- **`crate::parser::nfsv3::file`**: Used to access the `handle` function. This is necessary to parse the `file` field of the `READ` arguments, which is a variable-length opaque file handle specific to the NFSv3 protocol.
- **`crate::parser::primitive`**: Used to access the `u32` and `u64` functions. These are necessary to parse the `count` (unsigned 32-bit integer) and `offset` (unsigned 64-bit integer) fields of the `READ` arguments from the wire format.
- **`crate::vfs::read`**: Used to import the `Args` structure. This is the target type that the `args` function constructs and returns, representing the deserialized arguments ready for consumption by the VFS layer.
- **`crate::parser`**: Used to import the `Result` type alias. This ensures that the function returns the canonical error type used throughout the parser subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments of an NFSv3 `READ` procedure from a byte stream into a structured `read::Args` object.
- To act as a specific adapter that maps the sequential wire format of the `READ` procedure (File Handle, Offset, Count) to the field order of the `vfs::read::Args` struct.

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`, representing the stream of bytes containing the NFSv3 arguments.

Outputs:
- `Result<read::Args>`: A `Result` containing the parsed `read::Args` struct on success, or a `parser::Error` on failure.

Steps:
1. **File Handle Parsing**: The function calls `file::handle(src)` to read the file handle from the stream. This reads a length prefix followed by the handle bytes.
2. **Offset Parsing**: The function calls `u64(src)` to read the 64-bit offset from the stream.
3. **Count Parsing**: The function calls `u32(src)` to read the 32-bit count from the stream.
4. **Aggregation**: The function constructs the `read::Args` struct using the parsed values: `read::Args { file, offset, count }`.
5. **Return**: The constructed struct is wrapped in `Ok` and returned.

Edge Cases:
- **Insufficient Data**: If the `src` stream ends before all fields (handle, offset, count) can be read, the underlying calls to `file::handle`, `u64`, or `u32` will return an `Error::IO`, which is propagated up by the `?` operator.

Complexity:
- Time: O(1) relative to the logic in this module, as the fields are of fixed maximum size (though the file handle is variable-length, it is bounded by `NFS3_FHSIZE`).
- Space: O(1) stack space for the arguments.

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same `read::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle` Function**: This module relies on `file::handle` to correctly interpret the file handle portion of the message. The `file` module ensures that the handle size matches `NFS3_FHSIZE` and encapsulates the logic of reading the length prefix and the byte array.

- **From `nfs_mamont::parser::primitive`**:
 - **Integer Decoding**: This module relies on `u64` and `u32` to handle the extraction of big-endian integers from the stream. This abstracts away the byte-order manipulation required by the XDR standard.

- **From `nfs_mamont::vfs::read`**:
 - **`Args` Structure**: This module constructs the `Args` struct defined in the VFS layer. The fields `file`, `offset`, and `count` in this struct directly correspond to the parsed values, ensuring a 1:1 mapping between the wire format and the VFS interface.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a constructor for the `read::Args` entity defined in `nfs_mamont::vfs::read`.

Relations:
- **Construction**: The `args` function is the factory that builds a `vfs::read::Args` instance from raw bytes.

Global Invariants:
- **Field Order**: The wire format must strictly follow the order: File Handle, Offset, Count. This order is enforced by the sequence of function calls in the `args` function.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type defined in the parent `parser` module. Based on the code and dependencies, the most likely error variant returned here is `Error::IO` (wrapped `std::io::Error`) if the stream is too short.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used after every parsing call (`file::handle(src)?`, `u64(src)?`, `u32(src)?`). This ensures that if any component fails to parse, the error is immediately returned to the caller.

Recoverability:
- **Unrecoverable for the current message**: If parsing fails, the stream cursor is left at an indeterminate position relative to the message boundary. The caller (typically the RPC dispatcher) must discard the rest of the message or close the connection.

Panics:
- Allowed: No.
- Conditions: The code consists entirely of calls to fallible parsing functions and a struct construction. No `unwrap`, `expect`, or indexing operations that could panic are present.

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This module is used in order to **deserialize the specific arguments required for the NFSv3 `READ` procedure** from the network stream into a high-level Rust structure compatible with the server's Virtual File System (VFS). The system contains a parser hierarchy where `primitive` handles basic types and `file` handles complex file identifiers. This module sits at the procedure-specific layer, orchestrating these lower-level parsers to extract the exact data needed for a read operation.

A typical usage scenario of the system involves the RPC dispatcher receiving a request identified as an NFSv3 `READ` call. The dispatcher invokes the `args` function in this module, passing the network stream. The function reads the file handle (identifying *what* to read), the offset (identifying *where* to start), and the count (identifying *how much* to read). The resulting `read::Args` struct is then passed to the VFS implementation to perform the actual I/O.

Inside the system, the following things happen and they use this module:
- **Protocol Decoding**: The module enforces the specific wire format layout of the `READ` arguments. By calling `file::handle` first, then `u64`, then `u32`, it ensures the data is interpreted according to the NFSv3 specification (RFC 1813).
- **Type Safety**: The module transforms raw bytes into the strongly-typed `vfs::read::Args`. This allows the rest of the server logic to interact with file handles and offsets using typed structs rather than raw byte arrays, preventing logic errors.

Without this module, the RPC layer would need to contain the specific logic for parsing `READ` arguments, breaking the separation of concerns between protocol parsing and request handling. This module encapsulates the "READ argument" knowledge, ensuring that changes to the NFSv3 protocol or the VFS interface can be managed in a localized manner.