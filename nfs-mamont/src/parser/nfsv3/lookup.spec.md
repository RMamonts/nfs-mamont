<!-- SPEC_HASH: 32c9f5dbc54da9613352ef1bcbff929c838266b2c049be6a199fb3c5ed511fdb -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::lookup
Rust File: src/parser/nfsv3/lookup.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the generic source trait for the `src` parameter. This allows the parser to consume bytes from any source that implements the standard read interface (e.g., network streams, memory buffers).
- **`crate::parser::nfsv3::file`**: Used to access the `handle` and `file_name` parsing functions. These functions encapsulate the logic for reading XDR-encoded file handles and strings, ensuring that the specific parsing logic (including padding and validation) is reused consistently across the NFSv3 parser.
- **`crate::vfs::lookup`**: Used to import the `Args` structure. This structure defines the expected output format of the parser, which is subsequently used by the VFS layer to perform the actual lookup operation.
- **`crate::parser`**: Used to import the `Result` type alias. This ensures that the function returns the standardized error type used throughout the parser subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments of an NFSv3 `LOOKUP` procedure from a byte stream into a structured `lookup::Args` object.
- To act as a bridge between the generic XDR parsing primitives (provided by the `file` module) and the specific VFS interface for the `LOOKUP` operation.

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`. This represents the stream of bytes containing the XDR-encoded arguments.

Outputs:
- `Result<lookup::Args>`: A Result containing the parsed arguments (`lookup::Args`) or a `parser::Error` if the stream is malformed or incomplete.

Steps:
1. The function invokes `file::handle(src)` to read the file handle of the parent directory from the stream. This operation consumes the bytes representing the handle and validates its size.
2. The function invokes `file_name(src)` to read the filename string from the stream. This operation consumes the length-prefixed string and any necessary XDR padding, validating the string content.
3. If both operations succeed, the function constructs a `lookup::Args` struct using the retrieved handle and name and returns it wrapped in `Ok`.
4. If either operation fails (e.g., due to unexpected end of stream, invalid handle size, or invalid string characters), the error is propagated immediately via the `?` operator.

Edge Cases:
- **Unaligned Data**: The underlying `file_name` function expects XDR-compliant padding (4-byte boundaries). If the input stream ends before the padding bytes can be read (as tested in `test_lookup_unaligned_name`), the function returns an error.
- **Malformed Handle**: If the file handle in the stream does not match the expected size (`NFS3_FHSIZE`), the `file::handle` function returns an error, which terminates the parsing.

Complexity:
- Time: O(N), where N is the length of the filename string. The file handle parsing is O(1) as it has a fixed size.
- Space: O(N), where N is the length of the filename string, which is allocated within the returned `Args` struct.

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same `Args` struct or the same `Error`.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle(src)`**: This mechanism is responsible for reading the opaque file handle bytes. It ensures that the handle size matches the NFSv3 specification (64 bytes) and wraps the bytes in a `file::Handle` struct.
 - **`file_name(src)`**: This mechanism reads a length-prefixed string, enforces maximum length limits, checks for invalid characters (like path separators), and consumes XDR padding. It returns a validated `file::Name`.

- **From `nfs_mamont::vfs::lookup`**:
 - **`Args`**: This is the target data structure. The parser populates the `parent` field with the `file::Handle` and the `name` field with the `file::Name` obtained from the `file` module.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a factory for the `lookup::Args` entity defined in the `vfs::lookup` module.

Relations:
- **Transformation**: The `args` function transforms a byte stream (`src`) into a `lookup::Args` struct.

Global Invariants:
- **XDR Compliance**: The input stream must conform to the XDR encoding rules for the `LOOKUP` arguments (specifically, a fixed-length opaque handle followed by a variable-length string with padding).
- **Validation**: The returned `Args` struct is guaranteed to contain a valid `file::Handle` and `file::Name` because the construction relies on the fallible parsing functions from the `file` module.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type returned via the `Result` alias. Specific variants likely include:
 - `Error::IO`: If the stream ends unexpectedly.
 - `Error::BadFileHandle`: If the handle size is incorrect.
 - `Error::MaxElemLimit`: If the filename is too long.
 - `Error::EnumDiscMismatch`: (Less likely here, but possible if internal parsing fails).

Error Propagation Strategy:
- **Direct Propagation**: The module uses the `?` operator to propagate errors returned by `file::handle` and `file_name` directly to the caller. It does not perform custom error mapping or wrapping.

Recoverability:
- **Unrecoverable**: If parsing fails, the stream cursor may be left in an undefined state relative to the higher-level RPC message structure. The caller must typically discard the rest of the message or the connection.

Panics:
- Allowed: No.
- Conditions: The code consists entirely of fallible calls (`?`) and struct construction, neither of which panics under normal circumstances.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **deserialize the specific arguments for the NFSv3 `LOOKUP` procedure** from the network stream into a format that the server's Virtual File System (VFS) can process. The system contains a layered architecture where the RPC layer handles the transport, the parser layer handles the protocol-specific binary formats, and the VFS layer handles the actual file system logic. This module sits at the intersection of the parser and VFS layers for the `LOOKUP` operation.

A typical usage scenario of the system involves an NFS client requesting to look up a file within a directory. The RPC dispatcher receives the request, identifies the procedure as `LOOKUP`, and calls this `args` function. The function reads the directory handle and the filename from the stream. If successful, it returns a `lookup::Args` struct. The RPC layer then passes this struct to the VFS implementation, which uses the `parent` handle to locate the directory and the `name` to find the specific entry.

Inside the system, the following things happen and they use this module:
- **Protocol Decoding**: The module delegates the low-level byte manipulation to the `file` module. By calling `file::handle` and `file_name`, it ensures that the complex details of XDR (e.g., big-endian integers, 4-byte padding for strings) are handled correctly and consistently with other NFSv3 procedures.
- **Type Safety**: The module constructs the `lookup::Args` struct, which is strictly typed by the VFS. This ensures that the VFS receives validated data (e.g., the filename is guaranteed not to contain path separators or be excessively long) rather than raw bytes, reducing the risk of bugs or security issues in the file system backend.
- **Separation of Concerns**: This module isolates the logic for "how to read a LOOKUP request" from the logic for "how to perform a LOOKUP". This allows the VFS implementation to remain agnostic to the network wire format, while the parser remains agnostic to the specific storage backend.

Without this module, the RPC dispatcher would need to contain inline logic to parse `LOOKUP` arguments, leading to code duplication and a violation of the separation of concerns. This module provides a dedicated, reusable entry point for parsing this specific procedure.