<!-- SPEC_HASH: e049dbd47839bc56bfe1022a051183b8bb1710117f1894d39e8982c65741a319 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::write
Rust File: src/parser/nfsv3/write.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the input source (`src`), allowing the parser to consume bytes from any stream (e.g., TCP stream, memory buffer).
- **`crate::parser::nfsv3::file`**: This module is used to parse the file handle field within the WRITE arguments. The `handle` function specifically deserializes the opaque file identifier into a `file::Handle` struct.
- **`crate::parser::primitive`**: This module provides the low-level XDR (External Data Representation) deserialization primitives. Specifically, `u32` and `u64` are used to read the size and offset fields, while `variant` is used to deserialize the `StableHow` enum from its integer discriminant.
- **`crate::vfs::write`**: This module defines the target data structure `ArgsPartial` which holds the parsed metadata (file handle, offset, size, stability requirement). It also defines the `StableHow` enum which represents the data durability constraints requested by the client.
- **`crate::parser`**: This module provides the `Result` type alias and the `Error` enum used for error handling and propagation throughout the parsing process.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the fixed-length metadata arguments of an NFSv3 `WRITE` procedure call from a byte stream into a structured `ArgsPartial` object.
- To separate the parsing of operation arguments from the parsing of the opaque data payload, facilitating staged processing where the data buffer might be handled separately (e.g., via zero-copy mechanisms).

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the serialized NFSv3 WRITE arguments, positioned at the start of the argument structure.

Outputs:
- `Result<write::ArgsPartial>`: A result containing the parsed `ArgsPartial` structure or a `parser::Error` if deserialization fails.

Steps:
1. **File Handle Extraction**: The function invokes `file::handle(src)` to consume the file handle bytes from the stream and convert them into a `file::Handle` struct.
2. **Offset Parsing**: The function invokes `primitive::u64(src)` to read the 64-bit offset indicating where the write should begin.
3. **Size Parsing**: The function invokes `primitive::u32(src)` to read the 32-bit count of bytes to be written.
4. **Stability Requirement Parsing**: The function invokes the local helper `stable_how(src)`, which in turn calls `primitive::variant::<StableHow>(src)`. This reads the integer discriminant and maps it to the appropriate `StableHow` enum variant (`Unstable`, `DataSync`, or `FileSync`).
5. **Structure Construction**: The parsed fields are aggregated into the `write::ArgsPartial` struct, which is then wrapped in `Ok` and returned.

Edge Cases:
- **Invalid Enum Discriminant**: If the integer value for the stability requirement does not correspond to a valid `StableHow` variant (0, 1, or 2), the underlying `variant` function will return an `Error::EnumDiscMismatch`.
- **Short Stream**: If the stream ends before all fields are read, the underlying primitive parsers will return an `IO` error.

Complexity:
- Time: O(1). The function reads a fixed number of bytes (file handle size + 4 + 8 + 4 bytes) regardless of the input size.
- Space: O(1). The function allocates only the `ArgsPartial` struct and the internal `file::Handle`.

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `ArgsPartial` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - `handle(src) -> Result<file::Handle>`: This mechanism is critical for identifying the target file. It encapsulates the logic for reading the file handle length and the handle bytes itself, validating that the length matches the expected `NFS3_FHSIZE`.
- **From `nfs_mamont::parser::primitive`**:
 - `u64(src) -> Result<u64>`: Used to extract the write offset. It ensures the correct interpretation of big-endian integers defined by the XDR standard.
 - `u32(src) -> Result<u32>`: Used to extract the data size.
 - `variant<T>(src) -> Result<T>`: Used to parse the `StableHow` enum. It relies on the `FromPrimitive` trait to safely convert the raw integer discriminant from the wire into the Rust enum variant.
- **From `nfs_mamont::vfs::write`**:
 - `ArgsPartial`: This structure serves as the container for the parsed data. Its definition implies that the actual data buffer (`B: Buffer`) is excluded, allowing the parser to focus solely on the metadata header of the WRITE request.

---

## 4. Data Model

Entities:
- **`ArgsPartial`**: The primary output entity containing the metadata for a write operation (file handle, offset, size, stability requirement).
- **`StableHow`**: An enum entity representing the client's requirement for data persistence (Unstable, DataSync, FileSync).

Relations:
- **Composition**: `ArgsPartial` is composed of a `file::Handle`, a `u64` (offset), a `u32` (size), and a `StableHow` enum.

Global Invariants:
- The input stream `src` must be positioned at the beginning of the WRITE argument structure.
- The fields in the byte stream must appear in the specific order: File Handle, Offset, Count, Stable.
- The `StableHow` integer value must be 0, 1, or 2 to be successfully parsed.

## 5. Error Model

Error Types:
- **`parser::Error`**:
 - `IO(std::io::Error)`: Propagated from the underlying `Read` trait if the stream ends unexpectedly or an I/O fault occurs.
 - `EnumDiscMismatch`: Returned if the stability requirement integer is not a valid discriminant for `StableHow`.
 - `BadFileHandle`: Propagated from `file::handle` if the file handle length is invalid.

Error Propagation Strategy:
- Custom enum (`Result<T>`). The module uses the `?` operator to propagate errors returned by `file::handle`, `primitive::u64`, `primitive::u32`, and `primitive::variant`.

Recoverability:
- Non-recoverable for the current parsing operation. If parsing fails, the `ArgsPartial` cannot be constructed, and the calling RPC handler must abort processing the request.

Panics:
- Allowed: No.
- Conditions: The code does not perform any unwrapping or panicking. All potential failures are captured in the `Result`.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines only free-standing parsing functions.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
You MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to interpret the metadata portion of an incoming NFSv3 `WRITE` request, transforming raw bytes into a structured format that the Virtual File System (VFS) can act upon. The system contains a complex network stack that handles RPC requests, where the `WRITE` operation is unique because it consists of a fixed-size argument header followed by a variable-length, potentially large data payload.

A typical usage scenario of the system involves a client sending a request to write data to a file. The RPC dispatcher receives the byte stream. Before attempting to read or allocate memory for the large data payload, the system uses this module's `args` function to parse the `ArgsPartial`. This allows the server to inspect the file handle, offset, and size immediately. This separation is crucial for efficiency and security; it enables the system to validate the request parameters (e.g., checking if the file handle is valid or if the offset is sane) before committing resources to handle the data.

Inside the system, the following things happen and they use this module:
- **Early Validation**: The system parses the `StableHow` enum to determine if the client requires a synchronous write to disk (expensive) or an asynchronous write (fast). This metadata is available immediately after parsing `ArgsPartial`.
- **Resource Management**: By parsing the `size` field via `ArgsPartial`, the system can determine exactly how many bytes to expect in the subsequent data stream, allowing for precise buffer allocation or zero-copy strategies without reading past the end of the message.
- **Decoupling**: The system relies on this module to handle the XDR decoding specifics (byte order, alignment) for the arguments, ensuring that the VFS layer receives clean, type-safe Rust structs (`ArgsPartial`) rather than raw byte arrays.