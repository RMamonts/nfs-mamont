<!-- SPEC_HASH: 59c49c846717c7979ec80be236219a8d75a245668d7920d020224950b5ca5aaf -->
# Module Specification

Module: nfs_mamont::parser::mount::mnt
Rust File: src/parser/mount/mnt.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as a trait bound for the `src` parameter, allowing the function to consume bytes from any source that implements the standard reading interface (e.g., network streams or buffers).
- **`crate::consts::mount::MOUNT_DIRPATH_LEN`**: Used to provide the maximum size constraint for the directory path string. This ensures the parser does not allocate memory exceeding the limits defined by the MOUNT protocol specification.
- **`crate::mount::mnt::Args`**: Used as the return type. This struct represents the high-level, typed arguments for the MOUNT procedure that will be consumed by the service layer.
- **`crate::parser::primitive::string_max_size`**: Used to perform the low-level XDR (External Data Representation) decoding. It reads the length-prefixed string from the stream, handles padding, and enforces the `MOUNT_DIRPATH_LEN` limit.
- **`crate::parser::Result`**: Used as the return type alias, standardizing the error handling to use `crate::rpc::Error`.
- **`crate::rpc::Error`**: Used to define the error variant `Error::IO`. This is specifically used to wrap validation errors from the VFS layer (`std::io::Error`) into the parser's canonical error type.
- **`crate::vfs::file`**: Used to import the `file::Path` type. This type encapsulates a validated filesystem path, ensuring that the string parsed from the network conforms to the VFS layer's requirements (e.g., non-empty, valid UTF-8) before being wrapped in `Args`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments of the MOUNT protocol's MNT procedure from a raw byte stream into a strongly-typed Rust structure (`Args`).
- To enforce protocol-level constraints (maximum path length) and application-level validation (VFS path validity) in a single step during the parsing phase.

Inputs:
- `src`: A mutable reference to a type implementing `Read`. This represents the incoming network stream or buffer containing the XDR-encoded arguments.

Outputs:
- `Result<Args>`: A Result containing the parsed `Args` struct on success, or a `crate::rpc::Error` on failure.

Steps:
1. **String Extraction**: The function calls `string_max_size(src, MOUNT_DIRPATH_LEN)`. This reads a 32-bit length integer from the stream, verifies it is within the allowed limit, reads the corresponding bytes, and consumes any necessary padding to align to the next 4-byte boundary.
2. **Path Validation**: The raw `String` obtained from the stream is passed to `file::Path::new`. This constructor performs internal validation (e.g., checking if the path is empty or contains invalid characters) defined in the VFS layer.
3. **Error Mapping**: If `file::Path::new` returns a `std::io::Error` (indicating validation failure), the `map_err(Error::IO)` closure converts it into `crate::rpc::Error::IO`.
4. **Argument Construction**: If validation succeeds, the valid `file::Path` is wrapped inside the `Args` struct (`Args { dirpath: ... }`), which is then returned wrapped in `Ok`.

Edge Cases:
- **Protocol vs. VFS Limits**: The `MOUNT_DIRPATH_LEN` (protocol limit) might be larger than the VFS internal limit. If the string is within the protocol limit but invalid for the VFS (e.g., specific disallowed characters), `file::Path::new` will fail, resulting in an `Error::IO`.
- **Empty Path**: If the client sends an empty path (length 0), `file::Path::new` is expected to reject it (based on VFS invariants), resulting in an error.

Complexity:
- **Time**: O(N), where N is the length of the directory path string in bytes. This is dominated by the read operation and the UTF-8 validation inside `string_max_size` and `file::Path::new`.
- **Space**: O(N), as the function allocates a new `String` and a `file::Path` to hold the directory path.

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same `Args` or the same `Error`.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
 - **`string_max_size`**: This function is critical for handling the XDR encoding format. It abstracts away the details of reading the length prefix, the data bytes, and the padding bytes. It also provides the security mechanism of limiting allocation size via `max_size`.
- **From `nfs_mamont::vfs::file`**:
 - **`file::Path`**: This type provides the validation gateway. By using `file::Path::new`, the parser ensures that the data entering the system is not just syntactically correct XDR, but also semantically valid for the filesystem implementation (e.g., no null bytes, valid UTF-8).
- **From `nfs_mamont::rpc`**:
 - **`Error::IO`**: This variant serves as the catch-all for I/O and validation failures in this context. It allows the parser to treat a "bad path" from the VFS layer with the same severity as a "network read error".

---

## 4. Data Model

Entities:
- None defined in this module. The module defines a single parsing function that operates on types defined elsewhere (`Args`, `file::Path`).

Relations:
- N/A

Global Invariants:
- N/A

## 5. Error Model

Error Types:
- **`crate::rpc::Error`**: The function returns this error type via the `Result` alias.
 - Specifically, it can return errors propagated from `string_max_size` (e.g., `Error::IO` if the stream ends unexpectedly, `Error::MaxElemLimit` if the path is too long).
 - It can return `Error::IO` if `file::Path::new` fails validation.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used to propagate errors from `string_max_size` directly.
- **Mapping**: The `map_err(Error::IO)` method is used to convert `std::io::Error` (from VFS validation) into `crate::rpc::Error::IO`, unifying the error types.

Recoverability:
- **Unrecoverable for the Request**: If this function returns an `Err`, the MOUNT request arguments are malformed or invalid. The RPC layer will typically discard the request and send an error response to the client. The stream state may be compromised if the error occurred mid-read.

Panics:
- Allowed: No
- Conditions: The code uses safe parsing primitives (`string_max_size`) and checked constructors (`file::Path::new`). There are no `unwrap` or `expect` calls that would cause a panic.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **parse and validate the incoming MOUNT protocol requests** at the entry point of the server's request processing pipeline. The system contains a complex implementation of an NFS server where the MOUNT protocol is the mechanism by which a client obtains a file handle for a directory path. Before the server can perform any logic (like checking permissions or looking up the file handle), it must translate the raw bytes sent by the client into a structured, validated Rust object.

A typical usage scenario of the system involves a client sending a `MNT` request packet containing a directory path (e.g., "/export/home"). The RPC dispatcher receives the packet and identifies the procedure. It then invokes this `mount` function, passing the network stream. The function reads the path string, ensuring it doesn't exceed the protocol's maximum length (`MOUNT_DIRPATH_LEN`). Crucially, it also validates the path against the VFS layer's rules using `file::Path::new`. If the path is valid, it is wrapped in `Args` and passed to the `Mnt` service trait implementation. If invalid, an error is returned immediately, preventing malformed data from reaching the core server logic.

Inside the system, the following things happen and they use this module:
- **Security Enforcement**: By using `string_max_size` with `MOUNT_DIRPATH_LEN`, the system prevents a malicious client from sending a massive path string that could exhaust server memory (a DoS attack).
- **Data Sanitization**: By converting the raw string to `file::Path`, the system ensures that the path conforms to the expectations of the filesystem backend (e.g., valid UTF-8, no null bytes) before any filesystem operations are attempted.
- **Abstraction Layer**: This module acts as the adapter between the "physical" layer (XDR bytes on the wire) and the "logical" layer (VFS types). It allows the high-level `Mnt` service to deal with `file::Path` objects instead of raw byte arrays, significantly reducing complexity in the business logic.

Without this module, the parsing logic would likely be scattered in the RPC dispatcher or the service implementation, leading to a mix of concerns (network reading vs. filesystem validation) and increasing the risk of security vulnerabilities or inconsistent error handling.