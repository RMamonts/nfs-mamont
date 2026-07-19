<!-- SPEC_HASH: 7c32bb43b46c0753ced00c1242f39aabed5aef2308d8495b0d75374169405704 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::read
Rust File: src/parser/nfsv3/read.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the `src` parameter, allowing the parsing function to consume bytes from any source that implements the standard read interface (e.g., network streams or memory buffers).
- **`crate::parser::nfsv3::file`**: This module provides the `handle` function, which is used to parse the file handle field from the input stream. This is necessary because the file handle is a complex structure (length-prefixed byte array) specific to the NFSv3 protocol.
- **`crate::parser::primitive`**: This module provides the `u32` and `u64` functions, which are used to parse the `count` and `offset` fields respectively. These functions abstract the details of reading big-endian integers (XDR format) from the stream.
- **`crate::vfs::read`**: This module provides the `read::Args` structure, which serves as the target data type for the parsing operation. The function populates this structure with the extracted data to be passed to the VFS layer.
- **`crate::parser`**: This module provides the `Result` type alias and the `Error` enum, which are used for error handling and propagation throughout the parsing logic.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments of an NFSv3 `READ` procedure from a byte stream into a structured `read::Args` object.
- To act as a bridge between the raw network protocol format and the high-level VFS interface used by the server logic.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the serialized arguments for an NFSv3 `READ` operation.

Outputs:
- `Result<read::Args>`: A Result containing the parsed `read::Args` structure on success, or a `parser::Error` on failure.

Steps:
1. **File Handle Parsing**: The function calls `file::handle(src)` to parse the file handle from the beginning of the stream. This reads the length prefix and the subsequent handle bytes.
2. **Offset Parsing**: The function calls `u64(src)` to read the next 8 bytes as a 64-bit unsigned integer, representing the offset at which to start reading.
3. **Count Parsing**: The function calls `u32(src)` to read the next 4 bytes as a 32-bit unsigned integer, representing the number of bytes to read.
4. **Structure Construction**: The function constructs a `read::Args` struct instance using the parsed file handle, offset, and count, and wraps it in `Ok`.

Edge Cases:
- **Insufficient Data**: If the `src` stream ends before all required fields (file handle, offset, count) can be read, the underlying primitive parsers or the file handle parser will return an `Error::IO`, which propagates out of this function.
- **Invalid File Handle**: If the file handle data in the stream is malformed (e.g., incorrect length), `file::handle` will return a specific error (e.g., `Error::BadFileHandle`), which propagates.

Complexity:
- Time: O(1) regarding algorithmic complexity, though it depends on the I/O speed of reading a fixed number of bytes (handle size + 12 bytes for integers).
- Space: O(1) additional space (excluding the size of the `read::Args` struct itself and the internal file handle storage).

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `read::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle` function**: This mechanism reads a length-prefixed byte array from the stream and validates it against `NFS3_FHSIZE`. It is critical for extracting the identifier of the file to be read.
- **From `nfs_mamont::parser::primitive`**:
 - **`u64` function**: Reads a big-endian 64-bit integer. Used here to extract the file offset.
 - **`u32` function**: Reads a big-endian 32-bit integer. Used here to extract the read count.
- **From `nfs_mamont::vfs::read`**:
 - **`Args` struct**: The data container that holds the `file` handle, `offset`, and `count`. This structure is the input type for the VFS `Read` trait.

---

## 4. Data Model

Entities:
- **`read::Args`**: The primary output entity containing the arguments for the read operation.
 - `file`: A `file::Handle` identifying the target file.
 - `offset`: A `u64` indicating the starting byte position.
 - `count`: A `u32` indicating the number of bytes to read.

Relations:
- **Composition**: `read::Args` is composed of one `file::Handle`, one `u64`, and one `u32`.

Global Invariants:
- The byte stream `src` must be ordered as follows: file handle data (variable length), offset (8 bytes), count (4 bytes).
- The `offset` and `count` are interpreted as unsigned integers in network byte order (Big-Endian).

## 5. Error Model

Error Types:
- **`parser::Error`**:
 - `IO(std::io::Error)`: Propagated if the stream ends unexpectedly or an I/O error occurs during reading.
 - `BadFileHandle`: Propagated from `file::handle` if the file handle length is invalid.
 - Other variants from `file::handle` or primitive parsers may also propagate.

Error Propagation Strategy:
- Custom enum (`Result<T>`). The function uses the `?` operator to propagate errors returned by `file::handle`, `u64`, and `u32` directly to the caller.

Recoverability:
- Non-recoverable for the specific parsing operation. If parsing fails, the stream is likely in an invalid state, and the calling RPC handler should abort processing the request.

Panics:
- Allowed: No.
- Conditions: The code does not perform any unwrapping or panicking; all errors are returned via the `Result` type.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines a free-standing function and does not implement traits for types defined in this module.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to translate the raw, binary wire format of the NFSv3 `READ` procedure arguments into the structured, type-safe `read::Args` object required by the Virtual File System (VFS). The system contains a network stack that receives RPC requests as opaque byte streams. Without this module, the server would be unable to interpret the specific sequence of bytes representing the file handle, offset, and count sent by a client.

A typical usage scenario of the system involves the RPC dispatcher receiving a `READ` request. The dispatcher identifies the procedure number and invokes this `args` function, passing the network stream. The function parses the file handle (using `file::handle`), the offset (using `primitive::u64`), and the count (using `primitive::u32`). The resulting `read::Args` struct is then passed to the `Vfs::read` implementation (defined in `vfs::read`) to perform the actual data retrieval from the storage backend.

Inside the system, the following things happen and they use this module:
- **Protocol Decoding**: The system relies on this module to correctly interpret the XDR (External Data Representation) format specific to the `READ` arguments, ensuring that the file handle is validated and integers are read with the correct endianness.
- **Interface Adaptation**: The module adapts the low-level byte stream input to the high-level `vfs::read::Args` interface, decoupling the network parsing logic from the file system logic. This allows the VFS to operate on structured data without needing to understand the wire protocol details.