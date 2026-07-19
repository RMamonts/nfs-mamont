<!-- SPEC_HASH: 2123b5dfa9455b8a95b41304fd9fc9b3d91b8dd2b1931d50813ae56e742944d5 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::rm_dir
Rust File: src/parser/nfsv3/rm_dir.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in the `args` function. It allows the parser to read bytes from a generic stream (e.g., TCP stream or memory buffer).
- **`crate::parser::nfsv3::file`**: Used to import the `handle` and `file_name` functions. These functions encapsulate the logic for parsing the file handle (directory identifier) and the filename string from the XDR (External Data Representation) wire format.
- **`crate::vfs::rm_dir`**: Used to import the `Args` structure. This is the target type that the `args` function constructs and returns, representing the deserialized arguments ready for the VFS layer.
- **`crate::parser`**: Used to import the `Result` type alias. This ensures that the function returns the canonical error type used throughout the parser subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `RMDIR` procedure from a byte stream into a structured `rm_dir::Args` object.
- To act as a specific adapter that maps the wire format of a "remove directory" request to the internal VFS domain model.

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`. This represents the incoming network stream or buffer containing the raw XDR-encoded arguments.

Outputs:
- `Result<rm_dir::Args>`: A Result containing the parsed arguments on success, or a `parser::Error` on failure (e.g., invalid data, IO error).

Steps:
1. The function calls `file::handle(src)` to read the file handle of the parent directory from the stream. This consumes the bytes corresponding to the directory identifier.
2. The function calls `file_name(src)` to read the name of the subdirectory to be removed from the stream. This consumes the bytes corresponding to the filename string.
3. The function constructs a `vfs::DirOpArgs` struct using the parsed handle and name.
4. The function wraps the `vfs::DirOpArgs` in the `rm_dir::Args` struct (specifically in the `object` field).
5. The function returns `Ok(rm_dir::Args)`.

Edge Cases:
- **Stream Exhaustion**: If `src` ends prematurely before the handle or name can be fully read, the underlying `file` parsers will return an `IO` error, which propagates through this function.
- **Invalid Data**: If the handle length or filename format is invalid according to NFSv3/XDR rules, the `file` parsers will return a specific `parser::Error` (e.g., `BadFileHandle`, `MaxElemLimit`).

Complexity:
- **Time**: O(N), where N is the length of the filename string. The file handle is a fixed size (64 bytes), so parsing it is O(1).
- **Space**: O(N), where N is the length of the filename string, which is allocated and stored in the resulting `Args` struct.

Determinism:
- **Deterministic**: Given the same input byte stream, the function will always produce the same `Args` struct or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle(src)`**: This mechanism is critical for reading the 64-byte file handle that identifies the parent directory. It handles the XDR opaque data parsing, including reading the length prefix and the byte array.
 - **`file_name(src)`**: This mechanism is critical for reading the variable-length string that identifies the directory entry to be removed. It handles the XDR string parsing, including the length prefix and UTF-8 validation (via the VFS `Name` constructor).

- **From `nfs_mamont::vfs::rm_dir`**:
 - **`Args` struct**: This is the destination data structure. The module populates the `object` field (which is a `vfs::DirOpArgs`) to satisfy the interface expected by the VFS `RmDir` trait.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It constructs an instance of `rm_dir::Args` defined in the `vfs` module.

Relations:
- **Construction**: The `args` function constructs a `rm_dir::Args` instance.
- **Composition**: The constructed `rm_dir::Args` contains a `vfs::DirOpArgs`, which in turn contains a `file::Handle` and a `file::Name`.

Global Invariants:
- **Parsing Order**: The function strictly adheres to the NFSv3 wire format order for `RMDIR3args`: the directory file handle must appear before the filename.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type re-exported from `crate::parser`. This includes variants for IO errors, invalid file handles, and string validation errors.

Error Propagation Strategy:
- **Propagation**: The module uses the `?` operator to propagate errors returned by `file::handle` and `file_name`. It does not introduce new error types or perform custom error mapping.

Recoverability:
- **Unrecoverable for the Message**: If parsing fails, the `src` stream cursor may be left in an undefined state relative to the higher-level RPC message. Typically, the caller (the RPC dispatcher) must discard the rest of the message or close the connection.

Panics:
- **Allowed**: No.
- **Conditions**: The code consists entirely of function calls and struct construction. There are no `unwrap`, `expect`, or `panic!` calls.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- None.

---

## 7. Overview

This module is used in order to **deserialize the specific arguments for the NFSv3 `RMDIR` procedure** from the network stream into a format that the server's Virtual File System (VFS) can process. The system contains a modular parser architecture where each NFS procedure (e.g., `READ`, `WRITE`, `RMDIR`) has a dedicated module responsible for interpreting its specific binary layout. This module isolates the logic for understanding the `RMDIR` wire format.

A typical usage scenario of the system involves the RPC dispatcher receiving a request message identified as a `RMDIR` call. The dispatcher invokes the `args` function in this module, passing the incoming byte stream. The function reads the directory handle and the target name, constructs the `rm_dir::Args` struct, and returns it. The dispatcher then passes this struct to the VFS layer, which executes the actual directory removal logic.

Inside the system, the following things happen and they use this module:
- **Protocol Decoupling**: The VFS layer knows only about `rm_dir::Args` and does not care about XDR or byte ordering. This module handles the translation from the "on-the-wire" representation (XDR) to the "in-memory" representation (VFS structs).
- **Code Reuse**: By delegating the parsing of the file handle and filename to the `file` module, this module avoids duplicating logic for reading common data types, ensuring consistency across all NFSv3 procedure parsers.

Without this module, the RPC dispatcher would need to contain inline logic for parsing `RMDIR` arguments, or the parsing logic would be scattered, making the codebase harder to maintain and increasing the risk of protocol implementation errors. This module ensures that the specific parsing rules for `RMDIR` are encapsulated and testable in isolation.