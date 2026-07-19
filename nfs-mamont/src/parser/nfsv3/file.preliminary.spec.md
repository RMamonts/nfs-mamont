<!-- SPEC_HASH: 2fb7514d12f49246df77b022a3c2c23091e0dcf4fe1d12707fa79402634ac082 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::file
Rust File: src/parser/nfsv3/file.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for input sources (`src`), allowing the parsing functions to read bytes from any stream (e.g., network sockets, memory buffers).
- **`crate::consts::nfsv3::NFS3_FHSIZE`**: This constant is used to validate the size of the file handle being parsed. The parser ensures the incoming file handle size matches the NFSv3 specification (64 bytes).
- **`crate::parser::primitive`**: This module is used to perform low-level deserialization of primitive data types (integers, arrays, strings) from the byte stream. Functions like `u32`, `u64`, `array`, and `string_max_size` abstract the details of reading XDR (External Data Representation) formatted data.
- **`crate::parser::{Error, Result}`**: These types are used for error handling and propagation. The module converts specific parsing failures (like invalid file handles or enum discriminants) into the `Error` enum variants defined in the parser module.
- **`crate::vfs::file`**: This module provides the target data structures (e.g., `Handle`, `Type`, `Attr`, `Name`, `Path`) that the parsing functions populate. The module acts as a deserializer for these VFS types.
- **`crate::vfs::{MAX_PATH_LEN}`**: This constant is used to enforce the maximum length constraint when parsing file paths, ensuring they do not exceed system or protocol limits.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize raw byte streams conforming to the NFSv3 protocol into strongly-typed Rust structures defined in the `vfs::file` module.
- To validate protocol-specific constraints, such as file handle sizes and string length limits, during the parsing process.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream (e.g., TCP stream, byte slice) containing the serialized NFSv3 data.

Outputs:
- `Result<T>`: A result containing the parsed structure `T` (e.g., `file::Handle`, `file::Attr`) or a `parser::Error` if deserialization or validation fails.

Steps:
1. **Primitive Extraction**: Functions call primitive parsers (e.g., `u32`, `u64`) to extract basic fields from the stream in the order defined by the NFSv3 wire format.
2. **Structure Construction**: Complex structures (like `file::Attr`) are built by recursively parsing their constituent fields (type, mode, size, timestamps, etc.).
3. **Validation**:
   - **File Handles**: The `handle` function reads a length prefix and strictly checks if it equals `NFS3_FHSIZE`. If not, it returns `Error::BadFileHandle`.
   - **Enums**: The `r#type` function maps integer discriminants to `file::Type` variants. If the integer is not a valid variant (1-7), it returns `Error::EnumDiscMismatch`.
   - **Strings**: `file_name` and `file_path` use `string_max_size` to read variable-length strings, then pass them to `Name::new` or `Path::new`.
4. **Error Mapping**: The `map_validation_error` helper intercepts `io::Error` from string constructors. If the error indicates the input is "too long", it is converted to `Error::MaxElemLimit`; otherwise, it is wrapped in `Error::IO`.

Edge Cases:
- **Short Reads**: If the stream ends before the required bytes are read, the underlying `parser::primitive` functions will return an `IO` error, which propagates up.
- **Invalid Enum Values**: Integers outside the range 1-7 for file types result in `Error::EnumDiscMismatch`.
- **String Length Violations**: Strings exceeding `MAX_NAME_LEN` or `MAX_PATH_LEN` result in `Error::MaxElemLimit`.

Complexity:
- Time: O(N), where N is the size of the data structure being parsed (number of bytes read).
- Space: O(N) for the allocated structures (e.g., `String` for paths/names, `Vec` if used internally, though mostly fixed-size structs here).

Determinism:
- Deterministic. Given the same input byte sequence, the functions will always produce the same output structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
  - `u32`, `u64`, `u32_as_usize`: Used to read big-endian integers (standard for NFS/XDR).
  - `array`: Used to read fixed-size byte arrays (specifically for the file handle).
  - `string_max_size`: Used to read length-prefixed strings with a hard limit, preventing allocation attacks or buffer overflows.
- **From `nfs_mamont::vfs::file`**:
  - `Name::new` / `Path::new`: These constructors are assumed to perform validation (likely checking for empty strings or invalid characters) and return `io::Error` if validation fails. The parser relies on this to enforce semantic correctness beyond just length.
- **From `nfs_mamont::parser`**:
  - `Error` enum: The module utilizes specific variants like `BadFileHandle`, `EnumDiscMismatch`, and `MaxElemLimit` to signal distinct protocol failure modes to the caller.

---

## 4. Data Model

Entities:
- **Parsed Structures**: The functions produce instances of `file::Handle`, `file::Type`, `file::Attr`, `file::Time`, `file::Device`, `file::WccAttr`, `file::Name`, and `file::Path`.
- **Input Stream**: The `src` parameter acts as a cursor over a sequence of bytes.

Relations:
- **Composition**: `file::Attr` is composed of `file::Type`, `file::Device`, and multiple `file::Time` instances. `file::WccAttr` is composed of `u64` and `file::Time`.
- **Validation Dependency**: The validity of `file::Name` and `file::Path` entities depends on the constants `vfs::MAX_NAME_LEN` and `vfs::MAX_PATH_LEN`.

Global Invariants:
- The byte stream `src` must be positioned such that the next bytes correspond to the structure being parsed (e.g., if parsing `attr`, the next byte must be the file type).
- The integer `1` maps to `Regular` file, `2` to `Directory`, etc., as defined in the `r#type` function.
- A file handle is only valid if its length prefix matches `NFS3_FHSIZE`.

---

## 5. Error Model

Error Types:
- **`parser::Error`**:
  - `BadFileHandle`: Returned when the file handle length prefix does not match `NFS3_FHSIZE`.
  - `EnumDiscMismatch`: Returned when an integer discriminant for `file::Type` is not within the valid range (1-7).
  - `MaxElemLimit`: Returned when a parsed string (filename or path) exceeds the defined maximum length.
  - `IO(io::Error)`: Returned for generic I/O issues (e.g., unexpected end of stream) or if string validation fails for reasons other than length.

Error Propagation Strategy:
- Custom enum (`Result<T>`). Errors are propagated using the `?` operator. Specific `io::Error` instances from VFS constructors are intercepted and mapped to `parser::Error` variants via `map_validation_error`.

Recoverability:
- Generally non-recoverable for the specific parsing operation. If a structure cannot be parsed, the stream is likely desynchronized or corrupt, and the calling RPC handler should typically abort processing the request.

Panics:
- Allowed: No.
- Conditions: The code does not contain any `panic!`, `unwrap()`, or `expect()` calls in the parsing logic. All potential failures are handled via `Result`.

---

## 6. Traits

List which external traits this module implements:
- None. This module only defines free-standing functions and does not implement traits for types defined in this module.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to translate the raw, binary wire format of the NFSv3 protocol into the abstract, type-safe domain model used by the Virtual File System (VFS) within the `nfs_mamont` server. The system contains a complex network stack that receives RPC requests containing opaque byte arrays representing file handles, attributes, and paths. Without this module, the VFS would be unable to interpret these bytes, as it operates on structured Rust types like `file::Handle` and `file::Attr` rather than raw buffers.

A typical usage scenario of the system involves an RPC dispatcher receiving a `LOOKUP` request. The request payload contains a directory file handle and a filename. The dispatcher uses this module's `handle` function to convert the directory handle bytes into a `file::Handle` object and the `file_name` function to convert the filename bytes into a `file::Name` object. These objects are then passed to the VFS trait implementation (e.g., `Vfs::lookup`) to perform the actual file system operation.

Inside the system, the following things happen and they use this module:
- **Protocol Enforcement**: The `handle` function ensures that only file handles strictly adhering to the `NFS3_FHSIZE` (64 bytes) are accepted, rejecting malformed requests early.
- **Type Safety**: The `r#type` function maps integer file types to Rust enums, allowing the rest of the codebase to use pattern matching instead of magic numbers.
- **Resource Safety**: By using `string_max_size` and mapping "too long" errors to `MaxElemLimit`, the module prevents the allocation of excessively large strings, protecting the server from denial-of-service attacks involving malformed path lengths.