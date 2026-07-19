<!-- SPEC_HASH: c052102f83950958ce0c3d218f725c4870eb406b3145b436815e4f3671fa12ed -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::rename
Rust File: src/parser/nfsv3/rename.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in the `args` function. It allows the parser to consume bytes from a generic stream (e.g., network socket or buffer).
- **`crate::parser::nfsv3::file`**: Used to access the low-level parsing primitives `handle` and `file_name`. These functions are responsible for reading the specific XDR-encoded structures (file handles and filenames) that constitute the arguments of a RENAME request.
- **`crate::vfs::rename`**: Used to import the `Args` structure. This is the target type that the `args` function constructs and returns, representing the deserialized arguments in the domain model of the Virtual File System.
- **`crate::parser`**: Used to import the `Result` type alias, ensuring that the function returns errors consistent with the rest of the parsing subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the specific binary layout of the NFSv3 `RENAME` procedure arguments from a byte stream into a structured `vfs::rename::Args` object.
- To map the sequential wire format (FromDirHandle, FromName, ToDirHandle, ToName) to the semantic structure of a rename operation (Source, Target).

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`, representing the stream of bytes containing the RENAME arguments.

Outputs:
- `Result<rename::Args>`: A Result containing the parsed `rename::Args` structure or a `parser::Error` if the stream is malformed or cannot be read.

Steps:
1. **Parse Source Directory**: The function calls `file::handle(src)` to read the file handle of the source directory from the stream.
2. **Parse Source Name**: The function calls `file_name(src)` to read the name of the file/directory to be renamed from the source directory.
3. **Construct Source Arguments**: The parsed handle and name are combined into a `vfs::DirOpArgs` struct, assigned to the `from` field.
4. **Parse Target Directory**: The function calls `file::handle(src)` to read the file handle of the target directory from the stream.
5. **Parse Target Name**: The function calls `file_name(src)` to read the new name for the file/directory within the target directory.
6. **Construct Target Arguments**: The parsed handle and name are combined into a `vfs::DirOpArgs` struct, assigned to the `to` field.
7. **Return Result**: The `from` and `to` `DirOpArgs` are aggregated into `rename::Args`, which is wrapped in `Ok` and returned.

Edge Cases:
- **Stream Exhaustion**: If the stream ends prematurely while reading a handle or name, the underlying `file` parsers will return an `IO` error, which propagates through this module.
- **Invalid Data**: If the file handle size is incorrect or the filename contains invalid characters/lengths, the `file` module returns specific errors (`BadFileHandle`, `MaxElemLimit`), which are propagated immediately.

Complexity:
- **Time**: O(1) relative to the protocol logic (fixed number of fields), though dependent on the size of the variable-length strings and handles read by the `file` module.
- **Space**: O(1) stack space for the arguments structure.

Determinism:
- **Deterministic**: Given the same input byte stream, the function will always produce the same `rename::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **XDR Decoding**: The module relies on `file::handle` to correctly interpret the opaque file handle bytes (including the length prefix) and `file_name` to interpret the variable-length string (including length prefix and padding). This ensures the RENAME arguments conform to the External Data Representation (XDR) standard used by NFSv3.
 - **Validation**: The module delegates validation logic (e.g., checking if a handle is exactly 64 bytes, or if a name exceeds `MAX_NAME_LEN`) to the `file` module. This allows the `rename` module to focus solely on the structure of the arguments rather than the validity of the individual fields.

- **From `nfs_mamont::vfs::rename`**:
 - **Domain Mapping**: The module uses the `rename::Args` struct as the output container. This struct defines the semantic fields `from` and `to`, which map directly to the logical operation of renaming a file from one location to another.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a constructor for the `rename::Args` entity defined in `nfs_mamont::vfs::rename`.

Relations:
- **Transformation**: The `args` function transforms a linear byte stream into a hierarchical `rename::Args` structure.
- **Composition**: `rename::Args` is composed of two `vfs::DirOpArgs` instances (`from` and `to`), each of which is composed of a `file::Handle` and a `file::Name`.

Global Invariants:
- **Field Order**: The byte stream must contain the fields in the specific order: Source Directory Handle, Source Name, Target Directory Handle, Target Name. This order is mandated by the NFSv3 RFC 1813 specification for the `RENAME` procedure arguments.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type returned via the `Result` alias. Specific variants depend on the `file` module, but typically include:
 - `Error::IO`: Underlying read failure.
 - `Error::BadFileHandle`: Invalid file handle size.
 - `Error::MaxElemLimit`: Filename too long.
 - `Error::EnumDiscMismatch`: (Unlikely here, but possible if internal parsing logic changes).

Error Propagation Strategy:
- **Immediate Propagation**: The `?` operator is used after every call to `file::handle` and `file_name`. If any component fails to parse, the `args` function returns immediately with that error, preventing the consumption of further bytes in the stream for this specific procedure.

Recoverability:
- **Unrecoverable for the Procedure**: If parsing fails, the `rename::Args` cannot be constructed. The RPC layer must treat this as a malformed request and typically discard the message or send a garbage argument error response.

Panics:
- **Allowed**: No.
- **Conditions**: The code does not perform any unwrapping or operations that could panic; all errors are handled via the `Result` type.

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This module is used in order to **translate the raw binary payload of an NFSv3 RENAME request into the high-level, type-safe arguments required by the server's Virtual File System (VFS)**. The system contains a layered parsing architecture where low-level modules handle XDR details and high-level modules define the domain objects. This module sits at the procedure-specific layer, orchestrating the parsing of the RENAME operation's unique structure.

A typical usage scenario of the system involves the RPC dispatcher receiving a request with a procedure number indicating `RENAME`. The dispatcher invokes the `args` function in this module, passing the incoming byte stream. The function reads the source and target directory handles and filenames, validates them via the `file` module, and returns a `vfs::rename::Args` struct. This struct is then passed to the VFS implementation to perform the actual file system operation.

Inside the system, the following things happen and they use this module:
- **Protocol Compliance**: The module enforces the specific wire format of the RENAME arguments (FromDir, FromName, ToDir, ToName). By strictly adhering to this order, it ensures the server correctly interprets client requests according to the NFSv3 standard.
- **Abstraction**: The module hides the complexity of byte manipulation and XDR decoding from the VFS layer. The VFS layer receives a clean `Args` struct with validated `Handle` and `Name` objects, allowing it to focus on file system logic rather than network protocol details.

Without this module, the logic for parsing RENAME arguments would either be duplicated in the RPC dispatcher or mixed with generic parsing logic, leading to a violation of separation of concerns and increased risk of bugs in protocol handling. This module centralizes the "deserialization" logic specifically for the RENAME operation.