<!-- SPEC_HASH: c052102f83950958ce0c3d218f725c4870eb406b3145b436815e4f3671fa12ed -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::rename
Rust File: src/parser/nfsv3/rename.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the input source (`src`), allowing the parsing function to read bytes from any stream (e.g., network buffers, files).
- **`crate::parser::nfsv3::file`**: This module is used to parse the constituent parts of the rename arguments. Specifically, `file::handle` is used to deserialize the directory file handles, and `file_name` is used to deserialize the filenames (old and new).
- **`crate::vfs::rename`**: This module provides the target data structure `rename::Args` which aggregates the parsed source and target directory operations. The parser populates this structure to be passed to the VFS layer.
- **`crate::parser`**: This module provides the `Result` type alias (typically `Result<T, Error>`) used for error handling and propagation during parsing.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the raw byte stream of an NFSv3 `RENAME` procedure call into a structured `rename::Args` object suitable for consumption by the VFS layer.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the serialized arguments for the RENAME operation in XDR (External Data Representation) format.

Outputs:
- `Result<rename::Args>`: A result containing the parsed arguments or a `parser::Error` if deserialization fails.

Steps:
1. **Parse Source Directory**: The function calls `file::handle(src)` to read the file handle of the source directory from the stream.
2. **Parse Source Name**: The function calls `file_name(src)` to read the name of the file/directory to be renamed from the stream.
3. **Construct Source Arguments**: The parsed handle and name are wrapped into a `vfs::DirOpArgs` structure, assigned to the `from` field.
4. **Parse Target Directory**: The function calls `file::handle(src)` again to read the file handle of the target directory.
5. **Parse Target Name**: The function calls `file_name(src)` again to read the new name for the file/directory from the stream.
6. **Construct Target Arguments**: The parsed handle and name are wrapped into a `vfs::DirOpArgs` structure, assigned to the `to` field.
7. **Return Result**: The `rename::Args` struct containing both `from` and `to` fields is returned wrapped in `Ok`.

Edge Cases:
- **Stream Exhaustion**: If the stream ends prematurely while reading a handle or name, the underlying `file` module functions will return an `IO` error, which propagates.
- **Invalid Data**: If the file handle size is incorrect or the filename is invalid (e.g., too long), the `file` module functions return specific errors (`BadFileHandle`, `MaxElemLimit`), which propagate.

Complexity:
- Time: O(N), where N is the total size of the handles and filenames being read (fixed overhead for reading lengths and bytes).
- Space: O(N), for the allocated strings (filenames) and byte arrays (handles) within the returned `rename::Args`.

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `rename::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - `handle(src)`: Reads a length-prefixed file handle and validates that the length matches `NFS3_FHSIZE`. Returns `file::Handle` or `Error::BadFileHandle`.
 - `file_name(src)`: Reads a length-prefixed string, validates it against maximum length limits, and constructs a `file::Name`. Returns `Error::MaxElemLimit` if the string is too long.
- **From `nfs_mamont::vfs::rename`**:
 - `Args`: A structure containing `from` and `to` fields, both of type `vfs::DirOpArgs`. This structure acts as the data transfer object for the rename operation.

---

## 4. Data Model

Entities:
- **`rename::Args`**: The final output structure containing the arguments for the rename operation.
- **`vfs::DirOpArgs`**: An intermediate structure representing a directory operation, consisting of a directory handle and an entry name.
- **`file::Handle`**: An opaque identifier for a directory (parsed from bytes).
- **`file::Name`**: A validated string representing a filename.

Relations:
- **Composition**: `rename::Args` is composed of two `vfs::DirOpArgs` instances (`from` and `to`).
- **Composition**: `vfs::DirOpArgs` is composed of one `file::Handle` and one `file::Name`.

Global Invariants:
- The input stream must contain the data in the specific order: From Dir Handle, From Name, To Dir Handle, To Name.
- The parsed `from` and `to` components must successfully pass the validation checks defined in the `file` module (e.g., handle size, name length) for the `Args` struct to be created.

---

## 5. Error Model

Error Types:
- **`parser::Error`**: The module does not define its own errors but propagates those from the `file` module.
 - `BadFileHandle`: Indicates the parsed file handle length did not match the expected `NFS3_FHSIZE`.
 - `MaxElemLimit`: Indicates a parsed filename exceeded the maximum allowed length.
 - `IO(io::Error)`: Indicates a general read failure from the source stream.

Error Propagation Strategy:
- **Custom Enum (`Result`)**: Errors are propagated using the `?` operator. The `args` function acts as a combiner, passing errors from sub-parsers directly to the caller.

Recoverability:
- Non-recoverable for the specific parsing operation. If any part of the arguments fails to parse, the entire `args` call fails, and the stream state is undefined for subsequent parsing attempts by this function.

Panics:
- Allowed: No.
- Conditions: The code uses `?` for error propagation and does not call `unwrap`, `expect`, or `panic!`.

---

## 6. Traits

List which external traits this module implements:
- None. This module only defines a free-standing parsing function.

---

## 7. Overview

This module is used in order to translate the raw, binary wire format of the NFSv3 `RENAME` procedure arguments into the high-level, type-safe domain model used by the Virtual File System (VFS). The system contains a network stack that receives RPC requests as opaque byte streams. Without this module, the VFS layer would be unable to execute a rename operation because it requires structured Rust types (`rename::Args`) rather than raw byte buffers.

A typical usage scenario of the system involves an NFS client sending a request to rename a file from "old" to "new". The server receives the request payload, which contains the source directory handle, the old filename, the target directory handle, and the new filename. The dispatcher invokes this module's `args` function. This function reads the stream, validates the handles and names, and constructs a `rename::Args` object. This object is then passed to the VFS implementation (e.g., `Vfs::rename`), which performs the actual file system modification.

Inside the system, the following things happen and they use this module:
- **Protocol Decoding**: The module ensures that the specific sequence of data (handle, name, handle, name) defined by the NFSv3 specification is strictly followed.
- **Validation Delegation**: By delegating the parsing of handles and names to the `file` module, this module ensures that constraints like file handle size and filename length are enforced before the request reaches the VFS logic.
- **Abstraction**: It hides the details of XDR (External Data Representation) deserialization from the upper layers of the server, allowing the VFS to operate purely on domain concepts like `Args` and `DirOpArgs`.

**Assumptions:**
- The `crate::parser::Result` is an alias for `std::result::Result<T, crate::parser::Error>`.
- The `crate::vfs::rename::Args` struct is visible and accessible to this parser module.
- The `file::handle` and `file_name` functions consume the exact number of bytes required for their respective structures from the stream, advancing the cursor correctly for the subsequent reads.