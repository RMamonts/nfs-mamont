<!-- SPEC_HASH: e049dbd47839bc56bfe1022a051183b8bb1710117f1894d39e8982c65741a319 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::write
Rust File: src/parser/nfsv3/write.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in parsing functions. It allows the parser to consume bytes from a generic stream (e.g., network socket or buffer).
- **`crate::parser::nfsv3::file`**: Used to parse the file handle field. The `file::handle` function is invoked to deserialize the 64-byte NFSv3 file handle from the stream.
- **`crate::parser::primitive`**: Used to perform low-level XDR (External Data Representation) parsing. The functions `u32` and `u64` are called to extract the `size` and `offset` fields, and `variant` is used to parse the `StableHow` enum discriminant.
- **`crate::vfs::write`**: Used to define the target structure `write::ArgsPartial` and the enum `write::StableHow`. The module populates these types with the data read from the stream.
- **`crate::parser`**: Used to import the `Result` type alias, ensuring that parsing errors are reported consistently with the rest of the parser subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the fixed-length scalar arguments of an NFSv3 `WRITE` procedure from the wire format into the `write::ArgsPartial` structure.
- To separate the parsing of the operation metadata (file handle, offset, size, stability) from the parsing of the opaque data payload. This staged parsing allows the caller to inspect the `size` field before allocating memory for the data buffer.

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`, representing the stream of bytes being parsed.

Outputs:
- `Result<write::ArgsPartial>`: A `Result` containing the partially parsed arguments or a `parser::Error`.

Steps:
1. **File Handle Parsing**: The function calls `file::handle(src)` to read the file handle from the stream. This consumes the file handle length prefix and the handle bytes.
2. **Offset Parsing**: The function calls `primitive::u64(src)` to read the 64-bit offset at which the write should begin.
3. **Size Parsing**: The function calls `primitive::u32(src)` to read the 32-bit count of bytes to be written.
4. **Stability Parsing**: The function calls the internal helper `stable_how(src)`. This helper uses `primitive::variant::<StableHow>(src)` to read a 32-bit integer and map it to the `StableHow` enum (`Unstable`, `DataSync`, or `FileSync`).
5. **Aggregation**: The parsed fields are aggregated into the `write::ArgsPartial` struct, which is then wrapped in `Ok` and returned.

Edge Cases:
- **Invalid Enum Discriminant**: If the integer read for `StableHow` does not correspond to `0`, `1`, or `2`, the `variant` function will return `Error::EnumDiscMismatch`.
- **Malformed File Handle**: If the file handle in the stream is invalid (e.g., wrong size), `file::handle` will return an error, which propagates immediately.

Complexity:
- Time: O(1). The function reads a fixed number of bytes (handle + 4 + 8 + 4).
- Space: O(1). The function allocates only the stack space required for the `ArgsPartial` struct.

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same `ArgsPartial` or error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle`**: This function encapsulates the logic for reading and validating the 64-byte file handle. The current module relies on it to enforce the specific size constraints of NFSv3 handles.

- **From `nfs_mamont::parser::primitive`**:
 - **`u32` and `u64`**: These functions handle the extraction of big-endian integers from the stream. The current module relies on them to abstract away byte order and alignment details.
 - **`variant`**: This function reads a discriminant and attempts to map it to a Rust enum using `FromPrimitive`. The current module uses it to safely convert the raw stability requirement integer into the type-safe `StableHow` enum.

- **From `nfs_mamont::vfs::write`**:
 - **`ArgsPartial`**: This struct defines the contract for the metadata of a write request. The current module acts as the builder for this struct, ensuring that the fields are populated in the order defined by the NFSv3 protocol.
 - **`StableHow`**: This enum defines the valid stability levels. The current module ensures that only these specific semantic values are accepted from the network.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a transformer, converting a byte stream into the `write::ArgsPartial` entity defined in `nfs_mamont::vfs::write`.

Relations:
- **Transformation**: The `args` function transforms a sequence of bytes in the `Read` stream into an instance of `write::ArgsPartial`.

Global Invariants:
- **Field Order**: The fields in the byte stream must appear in the specific order: file handle, offset, size, stable how. This order is enforced by the sequence of function calls in `args`.
- **Partial State**: The returned `ArgsPartial` struct is intentionally incomplete; it lacks the `data` field (the actual bytes to write), which must be obtained by a subsequent read operation on the stream.

## 5. Error Model

Error Types:
- **`parser::Error`**: The primary error type used throughout the module.
 - `Error::IO`: Propagated from the underlying `Read` source if it fails to provide bytes.
 - `Error::EnumDiscMismatch`: Returned by `variant` if the stability integer is not 0, 1, or 2.
 - `Error::BadFileHandle`: Propagated from `file::handle` if the handle size is incorrect.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used after every parsing step. If any sub-parser (`file::handle`, `u64`, `u32`, `variant`) returns an `Err`, the `args` function terminates immediately and returns that error.

Recoverability:
- **Unrecoverable for the current operation**: If parsing fails, the stream cursor is left at an undefined position relative to the higher-level RPC message. The caller must typically discard the rest of the message or close the connection.

Panics:
- Allowed: No
- Conditions: The code uses only fallible operations (`?`) and does not call `unwrap` or `expect`.

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This module is used in order to **parse the control plane arguments of an NFSv3 WRITE request**, specifically separating the metadata from the data payload. The system contains a complex parser hierarchy where `primitive` handles raw bytes and `vfs::write` defines the semantic structures. This module sits in the middle, implementing the specific logic to read the file handle, offset, size, and stability requirement from the network stream.

A typical usage scenario of the system involves the RPC layer receiving a WRITE request. The RPC layer calls `args` to parse the initial fixed-size fields. It receives an `ArgsPartial` struct. The RPC layer then inspects the `size` field to determine how much memory to allocate for the data payload. It then reads exactly `size` bytes from the stream into that buffer. Finally, it combines the `ArgsPartial` and the buffer to form the full `Args` structure required by the VFS `Write` trait.

Inside the system, the following things happen and they use this module:
- **Staged Processing**: The module enables a two-phase parsing strategy. By returning `ArgsPartial` instead of the full `Args`, it allows the system to handle the potentially large data payload efficiently, allocating memory only after knowing the required size.
- **Type Safety**: The module uses the `variant` function to convert the raw integer stability flag into the `StableHow` enum. This ensures that the rest of the server logic deals with semantic concepts (`Unstable`, `DataSync`) rather than magic numbers.

Without this module, the parsing logic for the WRITE procedure would be embedded in the generic RPC dispatcher or mixed with buffer allocation logic, leading to a loss of separation of concerns. This module centralizes the knowledge of the WRITE argument layout, ensuring that the metadata is correctly extracted before the data is handled.