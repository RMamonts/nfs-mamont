<!-- SPEC_HASH: 1b5aec91fbfb4442574a5402b39c0ce294b8ff81a5955d7a0080fc1ad556a35c -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::read_dir_plus
Rust File: src/parser/nfsv3/read_dir_plus.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in all parsing functions. It allows the parser to consume bytes from a generic stream (e.g., a TCP stream or a buffer).
- **`crate::parser::nfsv3::file`**: Used to import the `handle` function. This is necessary to parse the directory file handle, which is a specific opaque data structure defined in the NFSv3 protocol.
- **`crate::parser::primitive`**: Used to import low-level XDR parsing functions (`u32`, `u64`, `array`). These functions handle the extraction of primitive data types (integers, fixed-size byte arrays) from the stream, ensuring correct endianness and alignment.
- **`crate::vfs::read_dir`**: Used to import the `Cookie` and `CookieVerifier` types. These types are the target wrappers for the parsed primitive values, providing type safety for the VFS layer.
- **`crate::vfs::read_dir_plus`**: Used to import the `Args` structure. This is the final aggregate structure that the `args` function constructs and returns.
- **`crate::parser`**: Used to import the `Result` type alias. This ensures that all parsing functions in this module return the canonical error type defined by the parser subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `READDIRPLUS` procedure from a raw byte stream into a structured Rust representation (`read_dir_plus::Args`).
- To provide type-safe constructors for `Cookie` and `CookieVerifier` by wrapping raw parsed values, bridging the gap between the wire format and the VFS domain model.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream representing the incoming RPC request payload.

Outputs:
- `Result<read_dir_plus::Args>`: The parsed arguments for the `READDIRPLUS` operation.
- `Result<read_dir::Cookie>`: A wrapper around a `u64` representing the directory offset.
- `Result<read_dir::CookieVerifier>`: A wrapper around an `[u8; 8]` array representing the directory state verifier.

Steps:
1. **Primitive Parsing (`cookie`)**: The `cookie` function calls `primitive::u64(src)` to read a 64-bit unsigned integer from the stream. It then wraps this value in `read_dir::Cookie::new(...)`.
2. **Verifier Parsing (`cookie_verifier`)**: The `cookie_verifier` function calls `primitive::array(src)` to read a fixed-size byte array (inferred to be 8 bytes based on the return type `read_dir::CookieVerifier`). It wraps this array in `read_dir::CookieVerifier::new(...)`.
3. **Argument Aggregation (`args`)**: The `args` function orchestrates the parsing of the full structure:
 - It calls `file::handle(src)` to parse the directory identifier.
 - It calls `cookie(src)` to parse the resume offset.
 - It calls `cookie_verifier(src)` to parse the state check.
 - It calls `primitive::u32(src)` to parse `dir_count` (the limit for directory info).
 - It calls `primitive::u32(src)` to parse `max_count` (the total response size limit).
 - It aggregates these fields into the `read_dir_plus::Args` struct and returns it.

Edge Cases:
- **Stream Exhaustion**: If the stream ends prematurely before all fields are read, the underlying `primitive` parsers will return an `IO` error, which propagates up.
- **Invalid Handle**: If the file handle parsed by `file::handle` is invalid (e.g., wrong length), an error is returned immediately, preventing the construction of `Args`.

Complexity:
- **Time**: O(1). The structure size is fixed and bounded by the protocol definition (handle size + 8 + 8 + 4 + 4 bytes).
- **Space**: O(1). The function allocates only the necessary stack structures for the `Args` and its components.

Determinism:
- **Deterministic**. Given the same input byte sequence, the functions will always produce the same output structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
 - **`u64`**: Used to read the raw 64-bit integer for the `cookie`. This handles the Big-Endian decoding required by XDR.
 - **`array`**: Used to read the fixed 8-byte opaque data for the `cookie_verifier`. This handles the reading of the exact byte count required by the verifier type.
 - **`u32`**: Used to read the `dir_count` and `max_count` fields, which are 32-bit unsigned integers in the protocol.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle`**: Used to parse the directory file handle. This function encapsulates the logic for reading the variable-length (but protocol-constrained) handle bytes and validating them against `NFS3_FHSIZE`.

- **From `nfs_mamont::vfs::read_dir`**:
 - **`Cookie` / `CookieVerifier`**: These types are used to wrap the parsed primitives. The module relies on the `new` constructors of these types to enforce any invariants (though the constructors appear to be simple wrappers in the provided context).

- **From `nfs_mamont::vfs::read_dir_plus`**:
 - **`Args`**: This is the target data structure. The module populates its fields (`dir`, `cookie`, `cookie_verifier`, `dir_count`, `max_count`) in the exact order defined by the struct.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a factory for entities defined in `nfs_mamont::vfs::read_dir` and `nfs_mamont::vfs::read_dir_plus`.

Relations:
- **Transformation**: The functions in this module transform a byte stream (`Read`) into high-level VFS argument structures (`read_dir_plus::Args`).

Global Invariants:
- **Field Order**: The `args` function must parse fields in the exact order specified by the NFSv3 RFC (dir, cookie, cookieverifier, dircount, maxcount) to match the wire format.
- **Verifier Size**: The `cookie_verifier` function implicitly relies on the `read_dir::CookieVerifier` type being an array of 8 bytes (`NFS3_COOKIEVERFSIZE`), matching the `array` parser's output.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type returned via the `Result` alias. This includes variants for IO errors, invalid data, and parsing failures.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used to propagate errors returned by `primitive` and `file` parsing functions. If any sub-parsing step fails, the `args` function returns that error immediately.

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

This module is used in order to **deserialize the arguments for the NFSv3 `READDIRPLUS` procedure**, enabling the server to interpret client requests for enhanced directory listings. The system contains a layered parser architecture where the `primitive` module handles raw byte extraction and the `file` module handles specific file system structures. This module sits at the procedure-specific layer, orchestrating these lower-level parsers to construct the `read_dir_plus::Args` structure required by the VFS.

A typical usage scenario of the system involves a client requesting a directory listing with attributes and handles (to avoid subsequent `LOOKUP` calls). The RPC dispatcher receives the byte stream and identifies the procedure as `READDIRPLUS`. It invokes the `args` function in this module. The function reads the directory handle, the cookie (for resuming), the verifier (for consistency), and size constraints. It then returns the fully populated `Args` struct.

Inside the system, the following things happen and they use this module:
- **Type Safety**: The module wraps raw integers and byte arrays into `Cookie` and `CookieVerifier` types. This ensures that the rest of the server (the VFS implementation) deals with strongly-typed concepts rather than raw `u64` or `[u8; 8]`, preventing logic errors.
- **Protocol Compliance**: By delegating to `primitive::u64` and `primitive::array`, the module ensures that the data is read according to XDR standards (Big-Endian, correct padding), which is critical for interoperability with standard NFS clients.
- **VFS Integration**: The module produces the exact `read_dir_plus::Args` structure expected by the `ReadDirPlus` trait defined in the VFS layer. This creates a clean contract: the parser produces `Args`, and the VFS consumes them.

Without this module, the RPC layer would lack the specific logic to decode `READDIRPLUS` requests, forcing it to mix generic parsing logic with procedure-specific field ordering, which would violate separation of concerns and make the codebase harder to maintain.