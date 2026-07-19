<!-- SPEC_HASH: 1b5aec91fbfb4442574a5402b39c0ce294b8ff81a5955d7a0080fc1ad556a35c -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::read_dir_plus
Rust File: src/parser/nfsv3/read_dir_plus.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the `src` parameter in all parsing functions, allowing the module to consume bytes from any source that implements the standard reading interface (e.g., network streams, memory buffers).
- **`crate::parser::nfsv3::file`**: This module is used to import the `handle` function. This function is necessary to parse the directory file handle, which is the first field in the `READDIRPLUS` arguments structure.
- **`crate::parser::primitive`**: This module provides low-level deserialization utilities (`u32`, `u64`, `array`) that are used to extract the basic data types (cookies, verifiers, and counts) from the byte stream according to the XDR (External Data Representation) standard.
- **`crate::parser::Result`**: This type alias is used as the return type for all public functions, standardizing error handling across the parser module.
- **`crate::vfs::read_dir`**: This module is used to import the `Cookie` and `CookieVerifier` types. These types are the target Rust structures for the parsed cookie and verifier data, serving as opaque markers for directory traversal state.
- **`crate::vfs::read_dir_plus`**: This module is used to import the `Args` structure. This is the final aggregate structure that the `args` function populates and returns, representing the complete set of arguments for the NFSv3 `READDIRPLUS` procedure.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the raw byte stream of an NFSv3 `READDIRPLUS` RPC call into a structured `read_dir_plus::Args` object.
- To provide specific parsing functions for the `Cookie` and `CookieVerifier` components, ensuring they are correctly wrapped in their VFS types.
- To validate the wire format implicitly by strictly adhering to the field order and types defined in the NFSv3 specification (RFC 1813).

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream. The stream position must be at the start of the `READDIRPLUS` arguments structure.

Outputs:
- `Result<read_dir_plus::Args>`: The parsed arguments structure containing the directory handle, cookies, and count limits.
- `Result<read_dir::Cookie>`: A wrapper around a `u64` representing the directory offset.
- `Result<read_dir::CookieVerifier>`: A wrapper around an 8-byte array representing the directory state signature.

Steps:
1. **Cookie Parsing**: The `cookie` function invokes `primitive::u64` to read 8 bytes from the stream (Big-Endian). It wraps the resulting integer in `read_dir::Cookie::new`.
2. **Verifier Parsing**: The `cookie_verifier` function invokes `primitive::array`. The size of the array (`N`) is inferred by the Rust compiler to be 8 bytes (`NFS3_COOKIEVERFSIZE`) based on the argument type expected by `read_dir::CookieVerifier::new`. It wraps the resulting array in `read_dir::CookieVerifier::new`.
3. **Argument Aggregation**: The `args` function performs the sequential parsing of the `READDIRPLUS` structure:
 - Calls `file::handle(src)` to parse the directory file handle.
 - Calls `cookie(src)` to parse the `cookie` field.
 - Calls `cookie_verifier(src)` to parse the `cookie_verifier` field.
 - Calls `primitive::u32(src)` to parse the `dir_count` field (maximum bytes of directory info).
 - Calls `primitive::u32(src)` to parse the `max_count` field (maximum total response size).
 - Constructs and returns the `read_dir_plus::Args` struct with these fields.

Edge Cases:
- **Type Inference for Array**: The `cookie_verifier` function relies on the compiler inferring the generic const `N` for `primitive::array`. If `read_dir::CookieVerifier` changes its internal array size, this code will fail to compile, ensuring strict synchronization with the VFS data model.
- **Stream Exhaustion**: If the `src` stream ends before all required bytes are read, the underlying `primitive` functions will return an `IO` error, which propagates up as `Err`.

Complexity:
- Time: O(1). The module parses a fixed-size structure (variable only by the file handle length, which is bounded).
- Space: O(1). Allocates only the necessary structures to hold the arguments.

Determinism:
- Deterministic. Given the same input byte sequence, the functions will always produce the same `Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
 - `u64`: Reads a Big-Endian unsigned 64-bit integer. Used for the `cookie` field.
 - `array<const N: usize>`: Reads a fixed-size byte array of length `N`. Used for the `cookie_verifier` field (where `N` is inferred as 8).
 - `u32`: Reads a Big-Endian unsigned 32-bit integer. Used for `dir_count` and `max_count`.
- **From `nfs_mamont::parser::nfsv3::file`**:
 - `handle`: Parses a variable-length file handle (length prefix followed by bytes). Used to identify the target directory in the `args` structure.
- **From `nfs_mamont::vfs::read_dir`**:
 - `Cookie`: A newtype wrapper around `u64`. The module uses `Cookie::new` to construct this type.
 - `CookieVerifier`: A newtype wrapper around `[u8; 8]`. The module uses `CookieVerifier::new` to construct this type.

---

## 4. Data Model

Entities:
- **`read_dir_plus::Args`**: The primary output structure containing `dir` (Handle), `cookie` (Cookie), `cookie_verifier` (CookieVerifier), `dir_count` (u32), and `max_count` (u32).
- **`read_dir::Cookie`**: An opaque identifier representing a position in a directory stream.
- **`read_dir::CookieVerifier`**: An opaque identifier representing the modification state of a directory.

Relations:
- **Composition**: `read_dir_plus::Args` aggregates `read_dir::Cookie` and `read_dir::CookieVerifier`.

Global Invariants:
- The `cookie_verifier` is always exactly 8 bytes long, enforced by the type signature of `read_dir::CookieVerifier` and the inferred generic parameter of `primitive::array`.
- The `cookie` is a `u64`, interpreted as an opaque value by the server but parsed as a standard integer here.

## 5. Error Model

Error Types:
- **`parser::Error`**: The specific variants depend on the underlying `primitive` and `file` parsers. Likely candidates include `IO` (unexpected end of stream) or `BadFileHandle` (if the handle parsing fails).

Error Propagation Strategy:
- Custom enum (`Result<T>`). The module uses the `?` operator to propagate errors returned by `file::handle`, `primitive::u64`, `primitive::u32`, and `primitive::array` directly to the caller.

Recoverability:
- Non-recoverable for the current RPC request. If parsing fails, the request is malformed, and the RPC layer should typically reject it without invoking the VFS.

Panics:
- Allowed: No.
- Conditions: The code consists entirely of parsing calls and struct construction. It does not perform any unchecked operations or manual indexing that could lead to a panic.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines only free-standing parsing functions.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to deserialize the arguments for the NFSv3 `READDIRPLUS` procedure, which allows clients to read directory contents while simultaneously retrieving file attributes and handles, optimizing performance by reducing subsequent `LOOKUP` calls. The system contains a complex parser subsystem that translates the raw XDR (External Data Representation) byte stream received over the network into the high-level Rust types used by the Virtual File System (VFS).

A typical usage scenario of the system involves an NFS client sending a request to list a directory. The RPC dispatcher receives the byte payload and invokes the `args` function from this module. This function interprets the bytes according to the NFSv3 wire format: it extracts the directory file handle (identifying *where* to read), the cookie (identifying *where* to start reading), the cookie verifier (ensuring the directory hasn't changed), and the count limits (controlling *how much* data to return).

Inside the system, the following things happen and they use this module:
- **Protocol Translation**: The `args` function acts as the adapter between the network layer (bytes) and the logic layer (VFS). It relies on `primitive::u64` and `primitive::array` to handle the byte-order and alignment requirements of XDR, ensuring that the opaque `cookie` and `cookie_verifier` are correctly extracted.
- **Type Safety**: By wrapping the raw integers and arrays in `read_dir::Cookie` and `read_dir::CookieVerifier`, the module ensures that the rest of the system treats these values as opaque tokens, preventing accidental misuse (e.g., arithmetic on cookies) outside of specific VFS implementations.
- **Dependency Coordination**: The module orchestrates calls to `file::handle` (for the directory) and the primitive parsers, ensuring that the complex `read_dir_plus::Args` structure is populated in the exact order required by the protocol specification. Without this module, the VFS would have no way to construct the arguments needed to initiate a `READDIRPLUS` operation.