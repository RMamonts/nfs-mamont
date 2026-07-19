<!-- SPEC_HASH: b1386653e81d9c41f6a7c61b71aa0a4aeda3900fec9ff5186d91290099e094cc -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::read_dir
Rust File: src/parser/nfsv3/read_dir.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in the `args` function. It allows the parser to consume bytes from a generic stream (e.g., a TCP stream or a buffer).
- **`crate::parser::nfsv3::file`**: Used to import the `handle` function. This is necessary to parse the directory file handle, which is a specific opaque data structure defined in the NFSv3 protocol.
- **`crate::parser::nfsv3::read_dir_plus`**: Used to import `cookie` and `cookie_verifier` functions. These are reused from the `READDIRPLUS` parser because the `READDIR` procedure uses identical fields for the directory offset and state verifier.
- **`crate::parser::primitive`**: Used to import the `u32` function. This handles the extraction of the 32-bit unsigned integer representing the `count` field (maximum response size) from the stream, ensuring correct endianness.
- **`crate::vfs::read_dir`**: Used to import the `Args` structure. This is the final aggregate structure that the `args` function constructs and returns, representing the input for the VFS layer's `ReadDir` operation.
- **`crate::parser`**: Used to import the `Result` type alias. This ensures that the parsing function returns the canonical error type defined by the parser subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `READDIR` procedure from a raw byte stream into a structured Rust representation (`read_dir::Args`).
- To orchestrate the parsing of composite types (file handle, cookie, verifier) by delegating to specialized sub-parsers, ensuring code reuse and consistency with the `READDIRPLUS` procedure.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream representing the incoming RPC request payload.

Outputs:
- `Result<read_dir::Args>`: The parsed arguments for the `READDIR` operation, containing the directory handle, resume cookie, verifier, and byte count limit.

Steps:
1. **Handle Parsing**: The `args` function calls `file::handle(src)` to parse the directory file handle from the stream. This reads the length and the opaque bytes.
2. **Cookie Parsing**: The function calls `cookie(src)` (imported from `read_dir_plus`) to read the 64-bit unsigned integer representing the directory offset.
3. **Verifier Parsing**: The function calls `cookie_verifier(src)` (imported from `read_dir_plus`) to read the 8-byte opaque verifier used to check directory consistency.
4. **Count Parsing**: The function calls `primitive::u32(src)` to read the 32-bit unsigned integer representing the maximum size of the directory response.
5. **Aggregation**: The parsed values are aggregated into the `read_dir::Args` struct, which is then wrapped in `Ok` and returned.

Edge Cases:
- **Stream Exhaustion**: If the stream ends prematurely before all fields are read, the underlying parsers will return an `IO` error, which propagates up.
- **Invalid Handle**: If the file handle parsed by `file::handle` is invalid (e.g., wrong length), an error is returned immediately, preventing the construction of `Args`.
- **Alignment Issues**: The test `test_readdir_unaligned_after_fh` implies that the parser relies on strict XDR alignment. If the file handle length is not a multiple of 4, the underlying padding logic (in `file` or `primitive`) expects specific padding bytes. If those bytes are missing or incorrect, the subsequent read for the cookie will fail or read garbage, resulting in a parsing error.

Complexity:
- **Time**: O(1). The structure size is fixed and bounded by the protocol definition (handle size + 8 + 8 + 4 bytes).
- **Space**: O(1). The function allocates only the necessary stack structures for the `Args` and its components.

Determinism:
- **Deterministic**. Given the same input byte sequence, the function will always produce the same output structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle`**: Used to parse the directory file handle. This function encapsulates the logic for reading the variable-length (but protocol-constrained) handle bytes and validating them against `NFS3_FHSIZE`.

- **From `nfs_mamont::parser::nfsv3::read_dir_plus`**:
 - **`cookie`**: Used to read the raw 64-bit integer for the `cookie`. This handles the Big-Endian decoding required by XDR.
 - **`cookie_verifier`**: Used to read the fixed 8-byte opaque data for the `cookie_verifier`. This handles the reading of the exact byte count required by the verifier type.

- **From `nfs_mamont::parser::primitive`**:
 - **`u32`**: Used to read the `count` field, which is a 32-bit unsigned integer in the protocol.

- **From `nfs_mamont::vfs::read_dir`**:
 - **`Args`**: This is the target data structure. The module populates its fields (`dir`, `cookie`, `cookie_verifier`, `count`) in the exact order defined by the struct.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a factory for the `Args` entity defined in `nfs_mamont::vfs::read_dir`.

Relations:
- **Transformation**: The `args` function transforms a byte stream (`Read`) into the high-level VFS argument structure (`read_dir::Args`).

Global Invariants:
- **Field Order**: The `args` function must parse fields in the exact order specified by the NFSv3 RFC (dir, cookie, cookieverifier, count) to match the wire format.
- **Verifier Size**: The `cookie_verifier` function implicitly relies on the `read_dir::CookieVerifier` type being an array of 8 bytes (`NFS3_COOKIEVERFSIZE`), matching the `array` parser's output.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type returned via the `Result` alias. This includes variants for IO errors, invalid data, and parsing failures.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used to propagate errors returned by `file`, `read_dir_plus`, and `primitive` parsing functions. If any sub-parsing step fails, the `args` function returns that error immediately.

Recoverability:
- **Unrecoverable for Message**: If parsing fails, the stream cursor is likely at an undefined position relative to the message boundary. The caller (typically the RPC dispatcher) must discard the rest of the message or close the connection.

Panics:
- **Allowed**: No.
- **Conditions**: The code consists entirely of parsing logic that returns `Result`. There are no `unwrap`, `expect`, or `panic!` calls.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
You MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **deserialize the arguments for the NFSv3 `READDIR` procedure**, enabling the server to interpret client requests for basic directory listings. The system contains a layered parser architecture where the `primitive` module handles raw byte extraction, the `file` module handles specific file system structures, and the `read_dir_plus` module provides shared logic for directory state tracking (cookies). This module sits at the procedure-specific layer, orchestrating these lower-level parsers to construct the `read_dir::Args` structure required by the VFS.

A typical usage scenario of the system involves a client requesting a directory listing. The RPC dispatcher receives the byte stream and identifies the procedure as `READDIR`. It invokes the `args` function in this module. The function reads the directory handle, the cookie (for resuming), the verifier (for consistency), and the size limit (`count`). It then returns the fully populated `Args` struct.

Inside the system, the following things happen and they use this module:
- **Code Reuse**: The module imports `cookie` and `cookie_verifier` from `read_dir_plus`. This is a critical design choice, as `READDIR` and `READDIRPLUS` share these exact fields. By reusing the logic, the system ensures that any changes to the parsing of these fields (e.g., strict validation) automatically apply to both procedures, reducing maintenance burden and preventing divergence.
- **Protocol Compliance**: By delegating to `primitive::u32` and `file::handle`, the module ensures that the data is read according to XDR standards (Big-Endian, correct padding), which is critical for interoperability with standard NFS clients.
- **VFS Integration**: The module produces the exact `read_dir::Args` structure expected by the `ReadDir` trait defined in the VFS layer. This creates a clean contract: the parser produces `Args`, and the VFS consumes them.

Without this module, the RPC layer would lack the specific logic to decode `READDIR` requests, forcing it to mix generic parsing logic with procedure-specific field ordering, which would violate separation of concerns and make the codebase harder to maintain.