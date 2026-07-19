<!-- SPEC_HASH: 9094b2afe2c0812dee8295fc4af7ea74b3171187ed10d682d0670a0e3b50f2b5 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::path_conf
Rust File: src/parser/nfsv3/path_conf.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in the `args` function. It allows the parser to read bytes from a generic stream (e.g., network socket or buffer).
- **`crate::parser::nfsv3::file`**: Used to access the `handle` function. This function is responsible for the actual deserialization of the file handle from the byte stream, including validation of its length.
- **`crate::parser::Result`**: Used as the return type for the `args` function. It wraps the `path_conf::Args` in a `Result` to handle potential parsing errors (e.g., IO errors, invalid data).
- **`crate::vfs::path_conf`**: Used to import the `Args` structure. This structure defines the expected output format of the parser, which will eventually be passed to the VFS layer for processing.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `PATHCONF` procedure from a raw byte stream into a structured `path_conf::Args` object.
- To delegate the low-level parsing of the file handle to the specialized `file` module, ensuring code reuse and consistency in handle validation.

Inputs:
- `src`: A mutable reference to a type implementing `std::io::Read`. This represents the stream of bytes containing the RPC arguments.

Outputs:
- `Result<path_conf::Args>`: A Result containing the parsed arguments if successful, or a `parser::Error` if the stream is malformed or cannot be read.

Steps:
1. **Handle Parsing**: The function calls `file::handle(src)` to read the file handle from the stream. This operation reads a length prefix followed by the handle bytes and validates the length against `NFS3_FHSIZE`.
2. **Error Propagation**: If `file::handle` returns an `Err` (e.g., due to an incorrect handle size or an IO error), the `?` operator immediately returns this error from `args`.
3. **Structure Construction**: If the handle is parsed successfully, it is used to construct an instance of `path_conf::Args`.
4. **Return**: The `Args` struct is wrapped in `Ok` and returned to the caller.

Edge Cases:
- **Malformed Handle**: If the byte stream does not contain a valid file handle (e.g., length mismatch), the error from `file::handle` is propagated.
- **Unexpected EOF**: If the stream ends before the handle can be fully read, an IO error is propagated.

Complexity:
- Time: O(1). The file handle has a fixed maximum size (`NFS3_FHSIZE`), so the read operation is bounded by a constant.
- Space: O(1). The `Args` struct contains a fixed-size handle.

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same `Args` struct or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle` function**: This is the primary mechanism used. It encapsulates the logic for reading the file handle length and the handle bytes itself. It ensures that the handle conforms to the NFSv3 specification (specifically `NFS3_FHSIZE`). By delegating to this function, the `path_conf` module ensures that it does not duplicate validation logic and that any changes to handle parsing are automatically reflected here.

- **From `nfs_mamont::vfs::path_conf`**:
 - **`Args` struct**: This defines the data model that the parser targets. The `Args` struct contains a single field, `file`, which is of type `file::Handle`. The parser's job is essentially to extract this handle from the wire format and place it into this struct.

---

## 4. Data Model

Entities:
- This module does not define any new public entities. It acts as a transformer, converting a byte stream into the `path_conf::Args` entity defined in the VFS layer.

Relations:
- **Transformation**: The `args` function transforms a `Read` stream into `path_conf::Args`.

Global Invariants:
- **NFSv3 Compliance**: The parsed arguments must conform to the `PATHCONF3args` structure defined in RFC 1813, which consists of a single file handle.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type returned via the `Result` alias. Specific variants depend on what `file::handle` returns (e.g., `Error::BadFileHandle`, `Error::IO`).

Error Propagation Strategy:
- **Direct Propagation**: The module uses the `?` operator to forward errors directly from `file::handle` to the caller. It does not perform any custom error mapping or wrapping.

Recoverability:
- **Unrecoverable for the current message**: If parsing fails, the stream cursor is likely in an undefined state relative to the RPC message boundary. The caller (typically the RPC dispatcher) must discard the message or close the connection.

Panics:
- Allowed: No
- Conditions: The code consists of a single call to a fallible function followed by a struct construction. No `unwrap`, `expect`, or `panic!` calls are present.

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than this level.

This module is used in order to **parse the incoming arguments for the NFSv3 `PATHCONF` procedure**, enabling the server to interpret client requests regarding file system configuration limits. The system contains a complex parser hierarchy where generic RPC handling is separated from specific procedure logic. This module represents the specific logic for the `PATHCONF` operation.

A typical usage scenario of the system involves the RPC dispatcher receiving a request message identified as a `PATHCONF` call. The dispatcher invokes this module's `args` function, passing the byte stream. The function reads the file handle from the stream. This handle identifies the file or directory the client is querying. The resulting `path_conf::Args` struct is then passed to the VFS layer, which uses the handle to look up the object and retrieve its specific limits (e.g., maximum filename length).

Inside the system, the following things happen and they use this module:
- **Protocol Decoding**: The module serves as the adapter between the raw XDR (External Data Representation) format on the wire and the internal Rust types. It ensures that the file handle is extracted correctly according to the NFSv3 specification.
- **Delegation of Complexity**: Instead of re-implementing file handle parsing (which involves reading length prefixes and validating sizes), this module delegates that task to the `file` module. This creates a clean separation of concerns: `path_conf` knows *what* needs to be parsed (a handle), and `file` knows *how* to parse it.

Without this module, the RPC dispatcher would lack a dedicated entry point for parsing `PATHCONF` arguments, leading to either scattered parsing logic or an inability to handle this specific NFSv3 procedure. This module ensures that the server can correctly interpret `PATHCONF` requests, which are essential for clients to adapt to the server's file system constraints.