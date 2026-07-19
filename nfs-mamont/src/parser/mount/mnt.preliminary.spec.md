<!-- SPEC_HASH: 59c49c846717c7979ec80be236219a8d75a245668d7920d020224950b5ca5aaf -->
# Module Specification

Module: nfs_mamont::parser::mount::mnt
Rust File: src/parser/mount/mnt.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used to define the input source as a generic stream of bytes. This allows the parser to operate on network sockets, memory buffers, or files interchangeably.
- **`crate::consts::mount::MOUNT_DIRPATH_LEN`**: Used to enforce the maximum allowed size for a directory path as defined by the MOUNT protocol specification. This prevents buffer overflows and ensures protocol compliance.
- **`crate::mount::mnt::Args`**: Used as the target data structure for the parsing operation. The function populates this struct to be passed to the higher-level mount logic.
- **`crate::parser::primitive::string_max_size`**: Used to perform the low-level extraction of a string from the byte stream while strictly enforcing the `MOUNT_DIRPATH_LEN` limit.
- **`crate::parser::Result`**: Used as the return type alias for the function, standardizing error handling across the parser module.
- **`crate::rpc::Error`**: Used to represent parsing failures. Specifically, `Error::IO` is used to wrap I/O errors that may occur during path validation.
- **`crate::vfs::file`**: Used to import the `Path` type. The parser converts the raw string into a `file::Path` to ensure the path is valid according to the Virtual File System's rules before passing it to the mount handler.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the MOUNT protocol's "MNT" procedure from a binary stream into a structured Rust type (`Args`).
- To enforce protocol-level constraints (maximum path length) immediately upon deserialization.
- To validate the semantic correctness of the path (via VFS `Path` creation) before the data reaches the business logic layer.

Inputs:
- `src`: A mutable reference to a type implementing the `Read` trait (e.g., a TCP stream or byte buffer), representing the raw payload of the RPC request.

Outputs:
- `Result<Args>`: A Result containing the parsed `Args` struct on success, or a `rpc::Error` on failure.

Steps:
1. **Read String**: The function invokes `string_max_size(src, MOUNT_DIRPATH_LEN)`. This reads a string from the stream, ensuring the byte length does not exceed the protocol limit.
2. **Validate Path**: The resulting `String` is passed to `file::Path::new(path)`. This constructor checks if the string is a valid filesystem path (e.g., valid UTF-8, no illegal characters).
3. **Error Mapping**: If `file::Path::new` returns an `Err` (an `io::Error`), it is mapped to `crate::rpc::Error::IO` using `map_err`.
4. **Construct Args**: If successful, the `file::Path` is wrapped in the `Args` struct (`Args { dirpath: ... }`).
5. **Return**: The `Args` struct is returned wrapped in `Ok`.

Edge Cases:
- **Path Too Long**: If the client sends a path exceeding `MOUNT_DIRPATH_LEN`, `string_max_size` is expected to return an error (assumed behavior based on name).
- **Invalid Path**: If the path contains invalid characters or is malformed, `file::Path::new` will fail, resulting in an `Error::IO`.

Complexity:
- Time: O(N), where N is the length of the directory path string in the stream. The function must read and validate each byte.
- Space: O(N), for allocating the intermediate `String` and the `file::Path`.

Determinism:
- Deterministic. Given the same byte sequence in `src`, the function will always produce the same `Args` struct or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::consts::mount`**:
    - **`MOUNT_DIRPATH_LEN`**: Provides the hard upper bound for the size of the directory path string. This is critical for preventing denial-of-service attacks via large payloads.
- **From `nfs_mamont::parser::primitive`**:
    - **`string_max_size`**: *Assumption*: This function reads a variable-length string (likely length-prefixed, typical of XDR used in NFS) from the stream. It ensures the read operation stops if the string size exceeds the provided `max_size` argument, returning a `Result<String, Error>`.
- **From `nfs_mamont::vfs::file`**:
    - **`Path::new`**: Acts as a validator. It transforms a raw `String` into a `Path` object, performing necessary checks (e.g., ensuring the string is not empty, contains valid characters for the OS/filesystem). It returns `io::Result<Self>`.
- **From `nfs_mamont::rpc`**:
    - **`Error::IO`**: The variant used to wrap standard I/O errors (like those from `Path::new`) so they fit the parser's error model.

---

## 4. Data Model

Entities:
- **`Args`**: A struct defined in `crate::mount::mnt`. It acts as a container for the parsed data.
    - `dirpath`: A `file::Path` instance representing the directory the client wishes to mount.

Relations:
- **Transformation**: The function transforms a stream of bytes (`src`) -> `String` -> `file::Path` -> `Args`.

Global Invariants:
- The `dirpath` field within the returned `Args` is guaranteed to be a valid `file::Path` instance.
- The length of the path string (in bytes) is guaranteed to be less than or equal to `MOUNT_DIRPATH_LEN`.

## 5. Error Model

Error Types:
- **`crate::rpc::Error`**: The module propagates errors from this enum.
    - `Error::IO`: Returned if the path string is invalid for the VFS (e.g., contains null bytes, invalid UTF-8) or if an underlying read error occurs.
    - *Assumption*: `string_max_size` may return other `rpc::Error` variants (like `MaxElemLimit` or `IncorrectString`) if the stream data is malformed.

Error Propagation Strategy:
- Propagation via the `?` operator. Errors from the primitive parser and the VFS layer are caught and converted (if necessary) into the unified `rpc::Error` type.

Recoverability:
- Generally non-recoverable for the specific RPC request. If parsing fails, the RPC layer must reject the request (e.g., with `GARBAGE_ARGS` status).

Panics:
- Allowed: No
- Conditions: The code uses `map_err` and `?` for error handling, avoiding explicit `panic!` or `unwrap()` calls on the `Result` types.

---

## 6. Traits

List which external traits this module implements:
- None. This module only defines a parsing function.

---

## 7. Overview

This module is used in order to deserialize the specific arguments required by the MOUNT protocol's "MNT" procedure from a raw network byte stream into a high-level Rust structure. In the context of the `nfs_mamont` NFS server, this module serves as the adapter between the wire format (XDR) and the internal Virtual File System (VFS) abstraction.

This system contains a parser stack that handles incoming RPC requests. When a client requests to mount a filesystem, it sends a directory path as a string. This module is responsible for extracting that string, ensuring it adheres to the size limits defined by the MOUNT protocol specification (RFC 1813), and validating that it represents a legitimate path within the server's VFS.

A typical usage scenario of the system involves the RPC dispatcher receiving a MOUNT request packet. After decoding the RPC header, the dispatcher identifies the procedure as "MNT" and invokes this `mount` function, passing the remaining payload bytes. The function reads the path, checks its length, and converts it into a `file::Path`. If successful, the `Args` struct is passed to the `Mnt` trait implementation to perform the actual mount logic (checking permissions, generating a file handle).

Inside the system, the following things happen and they use:
- **Protocol Enforcement**: Uses `MOUNT_DIRPATH_LEN` to ensure the client does not send a path larger than the server can handle or the protocol allows.
- **Data Validation**: Uses `file::Path::new` to ensure the path is semantically valid before the server attempts to access the filesystem, preventing errors deeper in the stack.
- **Abstraction**: Uses `parser::primitive::string_max_size` to handle the details of reading variable-length data from the stream, abstracting away the byte-level manipulation.