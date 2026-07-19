<!-- SPEC_HASH: b1386653e81d9c41f6a7c61b71aa0a4aeda3900fec9ff5186d91290099e094cc -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::read_dir
Rust File: src/parser/nfsv3/read_dir.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the `src` parameter in the `args` function, allowing the parser to consume bytes from any source that implements the standard reading interface.
- **`crate::parser::nfsv3::file`**: This module is used to import the `handle` function. This function is necessary to parse the directory file handle, which is the first field in the `READDIR` arguments structure.
- **`crate::parser::nfsv3::read_dir_plus`**: This module is used to import the `cookie` and `cookie_verifier` functions. These functions are reused here because the wire format for cookies and verifiers is identical between the `READDIR` and `READDIRPLUS` NFSv3 procedures.
- **`crate::parser::primitive`**: This module provides the `u32` function, which is used to extract the `count` field (maximum directory response size) from the byte stream according to the XDR standard.
- **`crate::parser::Result`**: This type alias is used as the return type for the public function, standardizing error handling across the parser module.
- **`crate::vfs::read_dir`**: This module is used to import the `Args` structure. This is the final aggregate structure that the `args` function populates and returns, representing the complete set of arguments for the NFSv3 `READDIR` procedure.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the raw byte stream of an NFSv3 `READDIR` RPC call into a structured `read_dir::Args` object.
- To reuse existing parsing logic from the `read_dir_plus` module for fields that share the same wire format (cookies and verifiers), avoiding code duplication.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream. The stream position must be at the start of the `READDIR` arguments structure.

Outputs:
- `Result<read_dir::Args>`: The parsed arguments structure containing the directory handle, cookie, cookie verifier, and byte count limit.

Steps:
1. **Directory Handle Parsing**: The `args` function calls `file::handle(src)` to parse the variable-length file handle identifying the target directory.
2. **Cookie Parsing**: The function calls `cookie(src)` (imported from `read_dir_plus`) to parse the `cookie` field, which represents the directory offset.
3. **Verifier Parsing**: The function calls `cookie_verifier(src)` (imported from `read_dir_plus`) to parse the `cookie_verifier` field, which ensures the directory state has not changed.
4. **Count Parsing**: The function calls `primitive::u32(src)` to parse the `count` field, which specifies the maximum size of the response in bytes.
5. **Aggregation**: The function constructs and returns the `read_dir::Args` struct with the parsed fields.

Edge Cases:
- **Invalid File Handle**: If the file handle length parsed by `file::handle` is invalid (e.g., does not match `NFS3_FHSIZE`), the function returns an error, as verified by the `test_readdir_unaligned_after_fh` test.
- **Stream Exhaustion**: If the `src` stream ends before all required bytes are read, the underlying primitive functions will return an `IO` error, which propagates up.

Complexity:
- Time: O(1). The module parses a fixed-size structure (variable only by the file handle length, which is bounded).
- Space: O(1). Allocates only the necessary structures to hold the arguments.

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::read_dir_plus`**:
 - `cookie`: Reads a Big-Endian `u64` and wraps it in `read_dir::Cookie::new`. Used to parse the directory offset.
 - `cookie_verifier`: Reads an 8-byte array and wraps it in `read_dir::CookieVerifier::new`. Used to parse the directory state signature.
- **From `nfs_mamont::parser::nfsv3::file`**:
 - `handle`: Parses a variable-length file handle (length prefix followed by bytes). Used to identify the target directory.
- **From `nfs_mamont::parser::primitive`**:
 - `u32`: Reads a Big-Endian unsigned 32-bit integer. Used to parse the `count` field.

---

## 4. Data Model

Entities:
- **`read_dir::Args`**: The primary output structure containing `dir` (Handle), `cookie` (Cookie), `cookie_verifier` (CookieVerifier), and `count` (u32).

Relations:
- **Composition**: `read_dir::Args` aggregates `file::Handle`, `read_dir::Cookie`, and `read_dir::CookieVerifier`.

Global Invariants:
- The `cookie_verifier` is always exactly 8 bytes long, enforced by the `read_dir_plus::cookie_verifier` parser.
- The `count` is a `u32` representing the maximum byte size of the response.

## 5. Error Model

Error Types:
- **`parser::Error`**: The specific variants depend on the underlying `file` and `primitive` parsers. Likely candidates include `IO` (unexpected end of stream) or `BadFileHandle` (if the handle parsing fails).

Error Propagation Strategy:
- Custom enum (`Result<T>`). The module uses the `?` operator to propagate errors returned by `file::handle`, `read_dir_plus::cookie`, `read_dir_plus::cookie_verifier`, and `primitive::u32` directly to the caller.

Recoverability:
- Non-recoverable for the current RPC request. If parsing fails, the request is malformed, and the RPC layer should typically reject it without invoking the VFS.

Panics:
- Allowed: No.
- Conditions: The code consists entirely of parsing calls and struct construction. It does not perform any unchecked operations or manual indexing that could lead to a panic.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines only a free-standing parsing function.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to deserialize the arguments for the NFSv3 `READDIR` procedure, which allows clients to read directory contents. Unlike `READDIRPLUS`, the `READDIR` procedure does not request file attributes or handles for the entries, resulting in a simpler argument structure (a single `count` field instead of `dircount` and `maxcount`). The system contains a parser subsystem that translates the raw XDR (External Data Representation) byte stream received over the network into the high-level Rust types used by the Virtual File System (VFS).

A typical usage scenario of the system involves an NFS client sending a request to list a directory. The RPC dispatcher receives the byte payload and invokes the `args` function from this module. This function interprets the bytes according to the NFSv3 wire format: it extracts the directory file handle (identifying *where* to read), the cookie (identifying *where* to start reading), the cookie verifier (ensuring the directory hasn't changed), and the count limit (controlling *how much* data to return).

Inside the system, the following things happen and they use this module:
- **Protocol Translation**: The `args` function acts as the adapter between the network layer (bytes) and the logic layer (VFS). It relies on `primitive::u32` to handle the byte-order requirements of XDR.
- **Code Reuse**: The module imports `cookie` and `cookie_verifier` parsers from the `read_dir_plus` module. This is a design optimization acknowledging that the definition of these fields is identical in both procedures, ensuring that changes to the wire format of cookies only need to be implemented in one place.
- **Dependency Coordination**: The module orchestrates calls to `file::handle` (for the directory) and the primitive parsers, ensuring that the complex `read_dir::Args` structure is populated in the exact order required by the protocol specification. Without this module, the VFS would have no way to construct the arguments needed to initiate a `READDIR` operation.