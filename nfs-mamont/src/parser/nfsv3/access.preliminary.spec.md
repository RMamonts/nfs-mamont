<!-- SPEC_HASH: 0903dd2f0526eab4cb2f0a6be5403ae309bd1ea4d5c7b3041dc14293344210bc -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::access
Rust File: src/parser/nfsv3/access.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the `src` parameter, allowing the parsing function to consume bytes from any source that implements the standard reading interface (e.g., network streams or memory buffers).
- **`crate::parser::nfsv3::file`**: This module is used to parse the file handle component of the arguments. Specifically, the `handle` function is invoked to read and validate the file handle from the byte stream.
- **`crate::parser::primitive::u32`**: This function is used to read the raw 32-bit unsigned integer representing the access mask from the byte stream.
- **`crate::vfs::access`**: This module provides the target data structure `Args` which the parser populates. It also provides the `Mask` type, specifically its `from_wire` method, to convert the raw integer into a sanitized access mask.
- **`crate::parser::Result`**: This type alias is used as the return type for the `args` function, encapsulating either the successfully parsed `access::Args` or a `parser::Error`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the specific arguments required for the NFSv3 `ACCESS` procedure from a raw byte stream into a structured Rust type (`access::Args`).
- To act as an adapter between the physical wire format (XDR) and the logical VFS interface, ensuring that the file handle is valid and the access mask is correctly sanitized before being passed to the storage layer.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the serialized arguments for an NFSv3 `ACCESS` call.

Outputs:
- `Result<access::Args>`: A Result containing the parsed arguments if successful, or a `parser::Error` if the stream is malformed or incomplete.

Steps:
1. **File Handle Parsing**: The function calls `file::handle(src)` to read the file handle from the stream. This operation validates that the handle size matches the NFSv3 specification (`NFS3_FHSIZE`).
2. **Mask Parsing**: The function calls `primitive::u32(src)` to read the next 4 bytes as a big-endian unsigned integer representing the requested access permissions.
3. **Mask Sanitization**: The raw `u32` value is passed to `access::Mask::from_wire`. This constructor applies a bitmask (`Mask::ALL`) to ensure that only valid protocol-defined bits are retained, ignoring any undefined bits set by the client.
4. **Argument Construction**: The parsed file handle and the sanitized mask are combined into an `access::Args` struct, which is then wrapped in `Ok` and returned.

Edge Cases:
- **Short Stream**: If the stream ends before the file handle or the mask can be fully read, the underlying `read_exact` calls in the dependency modules will return an `IO` error, which propagates through the `?` operator.
- **Invalid File Handle**: If the file handle length prefix does not match `NFS3_FHSIZE`, `file::handle` returns `Error::BadFileHandle`, which terminates the parsing immediately.
- **Undefined Access Bits**: If the client sets bits in the mask that are not defined in the NFSv3 specification (e.g., bits outside the lower 6 bits), `Mask::from_wire` silently drops them rather than returning an error.

Complexity:
- Time: O(1). The function reads a fixed amount of data (file handle size + 4 bytes).
- Space: O(1). The function allocates only the fixed-size `access::Args` struct and the internal `file::Handle`.

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `access::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle`**: Reads a length-prefixed byte array and strictly validates that the length equals `NFS3_FHSIZE` (64 bytes). This ensures that the parser rejects malformed file handles before they reach the VFS layer.
- **From `nfs_mamont::parser::primitive`**:
 - **`u32`**: Reads 4 bytes from the stream and interprets them as a Big-Endian (network byte order) unsigned 32-bit integer. This is essential for compliance with the XDR standard used in NFS.
- **From `nfs_mamont::vfs::access`**:
 - **`Mask::from_wire`**: Takes a raw `u32` and performs a bitwise AND with `Mask::ALL` (0x003F). This sanitization mechanism prevents undefined protocol bits from affecting internal logic, ensuring robustness against non-compliant clients.

---

## 4. Data Model

Entities:
- **`access::Args`**: The primary output entity containing the parsed arguments. It holds a `file::Handle` (identifying the target object) and a `Mask` (representing the requested permissions).

Relations:
- **Composition**: `access::Args` is composed of one `file::Handle` and one `Mask`.

Global Invariants:
- The `mask` field within the returned `Args` will always have a value where bits outside the range defined by `Mask::ALL` are zeroed out.
- The `file` field within the returned `Args` is guaranteed to have a length of `NFS3_FHSIZE` (64 bytes) if parsing succeeds.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type imported from the parent module. Specific variants likely returned include:
 - `Error::IO`: If the underlying stream fails to provide enough bytes.
 - `Error::BadFileHandle`: If the file handle length is incorrect.

Error Propagation Strategy:
- Custom enum (`Result<T>`). The module uses the `?` operator to propagate errors returned by `file::handle` and `primitive::u32` directly to the caller.

Recoverability:
- Non-recoverable for the current parsing operation. If parsing fails, the stream cursor may be in an undefined state (e.g., in the middle of a read), and the caller should typically abort processing the current RPC request.

Panics:
- Allowed: No.
- Conditions: The code consists entirely of function calls and struct construction. It does not perform any unwrapping or operations that could induce a panic.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines a single free-standing function and does not implement traits for any types.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to deserialize the specific payload of an NFSv3 `ACCESS` procedure call from the network wire format into a high-level representation suitable for the Virtual File System (VFS). The system contains a layered architecture where raw bytes are processed by primitive parsers, composed into protocol-specific structures (like file handles), and finally aggregated into operation-specific arguments. This specific module serves as the glue for the `ACCESS` operation, ensuring that the binary data received over the network is correctly interpreted as a file handle and a permission mask.

A typical usage scenario of the system involves an NFS client requesting permission to perform actions on a file. The RPC dispatcher receives the request bytes and identifies the procedure number as `ACCESS`. It invokes this module's `args` function, passing the byte stream. The function reads the file handle (identifying *what* is being accessed) and the mask (identifying *how* the client wants to access it). The resulting `access::Args` struct is then passed to the VFS implementation, which performs the actual permission check against the underlying storage (e.g., checking Unix mode bits or ACLs).

Inside the system, the following things happen and they use this module:
- **Protocol Compliance**: The system uses `file::handle` (via this module) to enforce the strict 64-byte size requirement for NFSv3 file handles, rejecting non-compliant requests immediately.
- **Input Sanitization**: The system uses `Mask::from_wire` (via this module) to strip out any undefined or malicious bits in the access request before they reach the VFS logic, preventing potential security issues or undefined behavior.
- **Separation of Concerns**: This module isolates the "serialization/deserialization" logic from the "business logic" (VFS). The VFS does not need to know that the file handle was preceded by a length field or that the mask was a big-endian integer; it simply receives a clean `Args` struct.