<!-- SPEC_HASH: af6ab1a7054271fc07b757464bd70a20f0694c4e423545df46020d13af0b5026 -->
# Module Specification

Module: nfs_mamont::parser::mount::umnt
Rust File: src/parser/mount/umnt.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as a trait bound for the `src` parameter, allowing the function to consume bytes from any source that implements the standard reading interface (e.g., a network stream or a byte buffer).
- **`crate::consts::mount::MOUNT_DIRPATH_LEN`**: Used to define the maximum allowed size (in bytes) for the directory path string. This constant is passed to the primitive parser to enforce protocol-level constraints and prevent potential denial-of-service attacks via excessively long strings.
- **`crate::mount::umnt::Args`**: Used as the return type. This struct represents the high-level, typed arguments required by the `Umnt` service trait, encapsulating the validated directory path.
- **`crate::parser::primitive::string_max_size`**: Used to perform the low-level XDR (External Data Representation) decoding. It reads the string length prefix, the string bytes, and any necessary padding from the stream, while enforcing the `MOUNT_DIRPATH_LEN` limit.
- **`crate::parser::Result`**: Used as the return type alias, standardizing error handling across the parser subsystem to return the specific `rpc::Error` type.
- **`crate::rpc::Error`**: Used to wrap I/O errors that may occur during the validation of the path string (specifically when constructing `file::Path`), converting them into the parser's canonical error type.
- **`crate::vfs::file::Path`**: Used to validate the semantic correctness of the parsed string. The constructor ensures the path is non-empty and conforms to VFS length limits before it is accepted as a valid argument for the unmount operation.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the MOUNT protocol's `UMNT` procedure (Procedure 3) from a raw byte stream into a structured, type-safe `Args` object.
- To enforce strict validation on the input data, combining protocol-level limits (XDR length) with application-level validation (VFS path rules) to ensure only valid data reaches the service layer.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded arguments.

Outputs:
- `Result<Args>`: A result containing the parsed `Args` struct on success, or a `rpc::Error` on failure.

Steps:
1. **Primitive Parsing**: The function calls `string_max_size(src, MOUNT_DIRPATH_LEN)` to read the directory path from the stream. This step handles the XDR format (reading the length prefix, the bytes, and padding) and ensures the raw byte length does not exceed the protocol-defined maximum.
2. **Error Propagation (Parsing)**: If the stream read fails (e.g., unexpected EOF) or the length exceeds `MOUNT_DIRPATH_LEN`, the error returned by `string_max_size` is propagated immediately via the `?` operator.
3. **Type Construction**: The raw `String` obtained from the stream is passed to `file::Path::new()`. This constructor validates the string against VFS rules (e.g., checking for emptiness or internal length limits).
4. **Error Mapping**: If `file::Path::new()` returns a `std::io::Error` (indicating validation failure), the error is mapped to `Error::IO` using `map_err`. This converts the VFS-specific error into the parser's generic error type.
5. **Result Packaging**: If validation succeeds, the valid `file::Path` is wrapped inside the `Args` struct, which is then returned wrapped in `Ok`.

Edge Cases:
- **Path Exceeds Protocol Limit**: If the client sends a path longer than `MOUNT_DIRPATH_LEN`, `string_max_size` returns an error, and the function returns `Err(Error::MaxElemLimit)`.
- **Path Exceeds VFS Limit**: If the path is within the protocol limit but violates VFS constraints (e.g., empty string), `file::Path::new` fails, and the function returns `Err(Error::IO(...))`.
- **Invalid UTF-8**: If the bytes read from the stream are not valid UTF-8, `string_max_size` returns an error, which is propagated.

Complexity:
- Time: O(N), where N is the length of the directory path string. This accounts for reading bytes from the stream and validating the string.
- Space: O(N), for storing the intermediate `String` and the resulting `file::Path`.

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same `Args` struct or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
 - **`string_max_size`**: This function is critical for handling the physical layer of the parsing. It abstracts away the details of XDR encoding (4-byte alignment, big-endian length) and provides a security boundary by enforcing the `max_size` limit. The current module relies on it to safely extract the raw string data.

- **From `nfs_mamont::vfs::file`**:
 - **`Path::new`**: This constructor provides the semantic validation layer. It ensures that the data extracted by the primitive parser is not just syntactically correct (valid XDR string) but also semantically valid for the server's file system logic (e.g., non-empty). The current module relies on this to guarantee that the `Args` struct contains a usable path.

- **From `nfs_mamont::rpc`**:
 - **`Error`**: The module relies on this enum to standardize failure reporting. By mapping `std::io::Error` to `Error::IO`, it ensures that the upper layers of the server (the RPC dispatcher) receive a consistent error type that they know how to handle (e.g., sending an RPC garbage args error or a system error response).

---

## 4. Data Model

Entities:
- **`Args`**: A structure defined in `crate::mount::umnt` acting as the carrier for the unmount arguments.
 - `dirpath`: `file::Path` — The validated server pathname of the directory to be unmounted.

Relations:
- **Composition**: The `unmount` function constructs an `Args` entity by composing a `file::Path` entity.

Global Invariants:
- **Validity**: The `Args` struct returned by this function is guaranteed to contain a `dirpath` that satisfies both the MOUNT protocol length constraints (`len <= MOUNT_DIRPATH_LEN`) and the VFS validity constraints (checked by `file::Path::new`).

## 5. Error Model

Error Types:
- **`crate::rpc::Error`**: The canonical error type for the parser.

Error Propagation Strategy:
- **Propagation**: The function uses the `?` operator to propagate errors returned by `string_max_size` (e.g., `IO`, `MaxElemLimit`, `IncorrectString`).
- **Mapping**: The function uses `map_err(Error::IO)` to convert `std::io::Error` (from `file::Path::new`) into `Error::IO`. This unifies the error handling path, ensuring that both parsing errors and validation errors are reported as `rpc::Error`.

Recoverability:
- **Unrecoverable for the Request**: If this function returns an `Err`, the arguments for the `UMNT` procedure are invalid or malformed. The RPC layer typically cannot proceed with the operation and must reject the request (e.g., by sending an RPC error response to the client).

Panics:
- Allowed: No
- Conditions: The code uses `map_err` and `?` for error handling. There are no `unwrap()`, `expect()`, or indexing operations that could cause a panic.

---

## 6. Traits

List which external traits this module implements:
- None.

List which external traits this module uses as bounds:
- **`std::io::Read`**: Used on the `src` parameter to allow reading from a generic stream.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **translate the raw network payload of a MOUNT protocol Unmount request into a validated, high-level Rust structure** that the server's service layer can act upon. The system contains a complex parser hierarchy where low-level modules handle the byte-level mechanics of XDR decoding, and high-level modules define the service interfaces. This module sits in the middle, orchestrating the decoding process for the specific `UMNT` procedure.

A typical usage scenario of the system involves an NFS client sending a request to unmount a directory. The RPC dispatcher identifies the procedure as `UMNT` and calls this `unmount` function. The function reads the directory path from the network stream. Crucially, it performs two layers of validation: first, it ensures the path isn't absurdly long (protecting the server from memory exhaustion via `MOUNT_DIRPATH_LEN`), and second, it ensures the path is structurally valid for the server's VFS (via `file::Path`). If both checks pass, the `Args` struct is passed to the `Umnt` service trait, which actually removes the mount entry from the server's state.

Inside the system, the following things happen and they use this module:
- **Protocol Enforcement**: The module ensures that the server strictly adheres to the MOUNT protocol specification by correctly interpreting the XDR string format and rejecting data that violates the defined size limits.
- **Security Boundary**: By using `string_max_size` with a hard limit, the module prevents malicious clients from sending massive strings that could crash the server or consume excessive memory.
- **Type Safety Bridge**: The module converts the "untyped" byte stream into a strongly-typed `file::Path`. This ensures that the service logic (the `Umnt` trait implementation) never has to deal with raw strings or worry about basic validation, allowing it to focus purely on the logic of removing the mount entry.

Without this module, the parser subsystem would lack a specific handler for the `UMNT` procedure, forcing the RPC layer to manually construct `Args` or rely on unsafe, unvalidated conversions, increasing the risk of bugs and security vulnerabilities.