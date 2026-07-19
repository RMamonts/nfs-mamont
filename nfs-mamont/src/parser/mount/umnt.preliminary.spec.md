<!-- SPEC_HASH: af6ab1a7054271fc07b757464bd70a20f0694c4e423545df46020d13af0b5026 -->
# Module Specification

Module: nfs_mamont::parser::mount::umnt
Rust File: src/parser/mount/umnt.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used to define the input source `src` as a generic stream of bytes. This allows the parser to operate on various underlying transports (e.g., TCP streams, memory buffers) that implement the standard `Read` trait.
- **`crate::consts::mount::MOUNT_DIRPATH_LEN`**: Used to provide the maximum allowed size for the directory path string as defined by the MOUNT protocol specification. This constant is passed to the parsing primitive to enforce protocol limits and prevent potential denial-of-service attacks via oversized payloads.
- **`crate::mount::umnt::Args`**: Used as the return type container. This struct represents the high-level, typed arguments for the Unmount operation that will be consumed by the server's business logic.
- **`crate::parser::primitive::string_max_size`**: Used to perform the low-level deserialization of the directory path from the byte stream. It handles reading the string length prefix, reading the actual bytes, and enforcing the `MOUNT_DIRPATH_LEN` constraint.
- **`crate::parser::Result`**: Used as the return type of the function. It is a type alias for `Result<T, crate::rpc::Error>`, standardizing error handling across the parser module.
- **`crate::rpc::Error`**: Used to define the error variants that can be returned during parsing. Specifically, `Error::IO` is used to wrap I/O errors or validation errors that occur during path construction.
- **`crate::vfs::file::Path`**: Used to wrap the raw directory string into a validated, type-safe path object. This ensures that the path conforms to the Virtual File System (VFS) expectations before being passed to the mount service.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the MOUNT protocol's "UMNT" (Unmount) procedure from a raw byte stream into a structured `Args` object. This involves reading a variable-length string, enforcing protocol-defined size limits, and validating the path format.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream (e.g., network socket) containing the serialized arguments.

Outputs:
- `Result<Args>`: A `Result` containing the parsed `Args` struct on success, or a `crate::rpc::Error` on failure.

Steps:
1. The function invokes `string_max_size(src, MOUNT_DIRPATH_LEN)` to read a string from the stream. This primitive reads a length prefix (typically a u32 in XDR), followed by that many bytes, ensuring the length does not exceed `MOUNT_DIRPATH_LEN`.
2. If the string is read successfully, the function attempts to convert the raw `String` into a `file::Path` using `file::Path::new(path)`. This step validates the path according to VFS rules.
3. If `file::Path::new` returns an `Err` (e.g., invalid path format), the error is mapped to `crate::rpc::Error::IO` using `map_err`.
4. The validated `file::Path` is wrapped inside the `Args` struct (`Args { dirpath: ... }`).
5. The `Args` struct is returned wrapped in `Ok`.

Edge Cases:
- **Oversized Path:** If the client sends a path length greater than `MOUNT_DIRPATH_LEN`, `string_max_size` will return an error, causing the parser to fail immediately.
- **Invalid Path:** If the string contains invalid characters or format for the VFS, `file::Path::new` will fail, resulting in an `Error::IO`.
- **Empty Stream/Unexpected EOF:** If the stream ends prematurely during the read operation, an `Error::IO` wrapping the underlying `std::io::Error` will be returned.

Complexity:
- Time: O(N), where N is the length of the directory path string. The time is dominated by reading bytes from the stream and validating the path.
- Space: O(N), for allocating the buffer to hold the path string and the `file::Path` object.

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::parser::primitive`**: The `string_max_size` function is critical. It abstracts the details of reading a length-prefixed string and enforcing a maximum size limit. It returns a `Result<String, Error>`, handling the conversion from raw bytes to a UTF-8 `String`.
- **From `crate::vfs::file`**: The `file::Path` struct and its `new` method are essential for validation. The `new` method acts as a constructor that performs sanity checks on the path string, returning an `io::Result`. This ensures that the `Args` struct cannot hold an invalid path.
- **From `crate::rpc`**: The `Error` enum, specifically the `IO` variant, is used to aggregate failures. Whether the failure comes from the network read (via `string_max_size`) or the path validation (via `file::Path::new`), it is normalized into the RPC error type.

---

## 4. Data Model

Entities:
- **`Args`**: A data structure defined in `crate::mount::umnt`. It acts as a container for the parsed arguments. It contains a single field `dirpath` of type `file::Path`.
- **`file::Path`**: A VFS entity representing a filesystem path. It encapsulates a `String` (or `PathBuf`) and guarantees that the content adheres to specific filesystem constraints.

Relations:
- **Composition**: The `Args` struct owns a `file::Path` instance.

Global Invariants:
- The `dirpath` field within the returned `Args` is guaranteed to be non-empty (unless the protocol allows empty strings, but `Path::new` usually validates structure) and strictly less than or equal to `MOUNT_DIRPATH_LEN` in byte length.

## 5. Error Model

Error Types:
- **`crate::rpc::Error`**: The primary error type returned.
    - `Error::IO`: Wraps `std::io::Error`. This is returned if the stream read fails or if `file::Path::new` fails validation.

Error Propagation Strategy:
- The function uses the `?` operator to propagate errors from `string_max_size` and `map_err` to convert errors from `file::Path::new`. All errors are ultimately converted into the `crate::rpc::Error` enum.

Recoverability:
- Not recoverable within this function. If parsing fails, the `Args` cannot be constructed, and the error must be handled by the caller (likely resulting in an RPC error response to the client).

Panics:
- Allowed: No
- Conditions: The function does not perform any operations that should panic (e.g., no array indexing with unchecked bounds, no `unwrap()` calls). All potential failure points (IO, validation) return `Result`.

---

## 6. Traits

List which external traits this module implements:
- None. This module only defines a parsing function and does not implement traits for types defined here.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to translate the raw byte payload of an NFS MOUNT protocol "Unmount" request into a structured, type-safe representation that the server's internal logic can process. The system containing this module is an NFS server implementation that must handle network requests from heterogeneous clients. These clients send binary data adhering to the XDR (External Data Representation) standard and the MOUNT protocol (RFC 1813).

The system requires a strict boundary between the "wire format" (bytes) and the "domain logic" (VFS operations). This module enforces that boundary by ensuring that any data passed to the unmount service has already been validated against protocol limits (e.g., `MOUNT_DIRPATH_LEN`) and VFS constraints (via `file::Path`).

A typical usage scenario involves a client sending a `UMNT` request to the server to release a previously mounted directory. The RPC layer receives the packet and extracts the argument bytes. The `unmount` function in this module is called with a reference to those bytes. It reads the directory path, checks that it isn't absurdly long (preventing memory exhaustion attacks), and validates that it looks like a real path. If successful, it returns an `Args` struct. This struct is then passed to the `Umnt` trait implementation (defined in `crate::mount::umnt`), which performs the actual removal of the mount entry from the server's state.

Inside the system, the following things happen and they use this module: The parser acts as a gatekeeper. It prevents malformed or malicious data from reaching the core server logic. By mapping `file::Path` construction errors to `rpc::Error::IO`, it ensures that the RPC layer can generate a standard error response (like `GARBAGE_ARGS`) if the client sends invalid data, maintaining the robustness of the server.