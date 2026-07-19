<!-- SPEC_HASH: 32c9f5dbc54da9613352ef1bcbff929c838266b2c049be6a199fb3c5ed511fdb -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::lookup
Rust File: src/parser/nfsv3/lookup.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a bound for the `src` parameter, allowing the parsing function to consume bytes from any generic input stream (e.g., network socket or memory buffer).
- **`crate::parser::nfsv3::file`**: This module is used to perform the actual deserialization of the specific fields contained in the `LOOKUP` arguments. Specifically, `file::handle` is used to parse the parent directory file handle, and `file_name` is used to parse the target filename.
- **`crate::vfs::lookup`**: This module is used to import the `Args` structure, which serves as the target domain object that the parsing function populates and returns.
- **`crate::parser`**: This module is used to import the `Result` type alias, which standardizes error handling across the parsing subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the byte stream representing the arguments of an NFSv3 `LOOKUP` procedure into the structured `lookup::Args` type used by the VFS layer.
- To act as a bridge between the raw XDR (External Data Representation) wire format and the high-level Rust types representing file system objects.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream. The stream position must be at the start of the `LOOKUP` argument structure.

Outputs:
- `Result<lookup::Args>`: On success, returns a `lookup::Args` struct containing the parsed parent directory handle and filename. On failure, returns a `parser::Error` (e.g., due to invalid data or unexpected end of stream).

Steps:
1. **Parse Parent Handle**: The function calls `file::handle(src)`. This reads the file handle bytes from the stream, validates the length (must be `NFS3_FHSIZE`), and constructs a `file::Handle`.
2. **Parse Filename**: The function calls `file_name(src)`. This reads the length-prefixed filename string from the stream, validates it (e.g., length limits, character validity), and consumes any necessary XDR padding bytes to align the stream to a 4-byte boundary.
3. **Construct Arguments**: If both parsing operations succeed, the function constructs the `lookup::Args` struct using the obtained handle and name and returns it wrapped in `Ok`.
4. **Error Propagation**: If either `file::handle` or `file_name` returns an error, the `args` function immediately returns that error to the caller using the `?` operator.

Edge Cases:
- **Unaligned/Padded Data**: The underlying `file_name` parser expects XDR padding. If the stream contains a valid string but is truncated before the padding bytes required to align to the next 4-byte boundary, the function will return an `IO` error (as demonstrated by `test_lookup_unaligned_name`).
- **Invalid Handle**: If the file handle length prefix does not match the expected `NFS3_FHSIZE`, `file::handle` returns a `BadFileHandle` error.
- **Invalid Filename**: If the filename exceeds maximum length or contains invalid characters, `file_name` returns a `MaxElemLimit` or `IO` error.

Complexity:
- Time: O(N), where N is the size of the file handle (fixed) plus the length of the filename string.
- Space: O(N), where N is the length of the filename string (stored in the returned `Args` struct). The file handle is fixed size.

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `Args` struct or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **Primitive Extraction**: The `handle` and `file_name` functions utilize primitive parsers to read big-endian integers and length-prefixed data from the stream.
 - **Validation**: The `handle` function strictly validates that the file handle size matches `NFS3_FHSIZE`. The `file_name` function enforces maximum length constraints and validates the string content (e.g., no slashes).
 - **Padding Handling**: The `file_name` function (via `string_max_size`) consumes XDR padding bytes to ensure the stream remains aligned for subsequent reads. This is critical for the `args` function to work correctly in a sequence of operations.

---

## 4. Data Model

Entities:
- **`lookup::Args`**: A structure representing the input arguments for a VFS lookup operation.
 - `parent`: `file::Handle` (The directory in which to search).
 - `name`: `file::Name` (The name of the entry to look up).

Relations:
- **Aggregation**: The `args` function aggregates a `file::Handle` and a `file::Name` into a single `lookup::Args` structure.

Global Invariants:
- **Stream Alignment**: Upon successful return of the `args` function, the input stream `src` is advanced past the arguments and aligned to a 4-byte boundary, ready for the next potential XDR structure.
- **Name Validity**: The `name` field in the returned `Args` is guaranteed to be a valid `file::Name` (non-empty, no path separators) as enforced by the `file_name` parser.

## 5. Error Model

Error Types:
- **`parser::Error`**: This enum wraps various failure modes, including:
 - `IO(std::io::Error)`: For stream read failures or unexpected end of stream (e.g., missing padding).
 - `BadFileHandle`: If the parsed handle length is incorrect.
 - `MaxElemLimit`: If the filename is too long.

Error Propagation Strategy:
- Custom enum (`Result<T>`). Errors are propagated directly from the helper functions `file::handle` and `file_name` using the `?` operator.

Recoverability:
- Non-recoverable for the current parsing context. If parsing fails, the stream state is likely compromised or partially consumed, and the caller should abort processing the current request.

Panics:
- Allowed: No.
- Conditions: The function does not perform any operations that can panic; all potential failure conditions (IO errors, validation errors) are captured in the `Result`.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines a free-standing function and does not implement traits for external types.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to convert the raw byte payload of an NFSv3 `LOOKUP` RPC request into a structured, type-safe argument object that the Virtual File System (VFS) can process. The system consists of a network server that receives binary streams encoded in XDR format. These streams contain opaque data representing file handles and filenames. Without this module, the higher-level VFS logic would be burdened with the low-level details of parsing wire formats (endianness, padding, length prefixes), violating separation of concerns and increasing the risk of implementation errors.

A typical usage scenario of the system involves an RPC dispatcher receiving a `LOOKUP` request. The dispatcher extracts the argument bytes from the RPC message and passes them to this module's `args` function. The function interprets the bytes: first reading the directory handle (ensuring it is exactly 64 bytes) and then reading the filename (ensuring it is valid and properly padded). The resulting `lookup::Args` struct is then passed to the VFS implementation (e.g., `Vfs::lookup`).

Inside the system, the following things happen and they use this module:
- **Protocol Enforcement**: The module ensures that the data strictly adheres to the NFSv3 specification. For instance, it relies on `file::handle` to reject file handles that are not 64 bytes long, preventing malformed data from reaching the storage backend.
- **Stream Management**: By delegating to `file_name`, the module ensures that XDR padding bytes are consumed. This is critical because the NFS protocol requires data structures to be aligned on 4-byte boundaries. If this module failed to consume padding, the stream cursor would be misaligned for any subsequent data reads, leading to corruption in later processing stages.
- **Type Safety**: The module transforms raw bytes into Rust types (`file::Handle`, `file::Name`). This allows the VFS to operate on guaranteed-valid types rather than raw byte arrays, leveraging the compiler to prevent invalid operations (e.g., treating a handle as a string).