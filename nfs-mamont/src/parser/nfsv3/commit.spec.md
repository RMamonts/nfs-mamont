<!-- SPEC_HASH: dbc8ec2522fad4c1eaafbdc0a4d8d4817007ba4ad9160987052fff11ddddc2db -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::commit
Rust File: src/parser/nfsv3/commit.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in the `args` function. It allows the parser to read bytes from a generic stream (e.g., a network socket or a buffer).
- **`crate::parser::nfsv3::file`**: Used to access the `handle` function. This function is responsible for parsing the file handle (an opaque identifier) from the stream, which is the first field of the `COMMIT` arguments.
- **`crate::parser::primitive`**: Used to access the `u32` and `u64` functions. These functions parse the `count` and `offset` fields respectively from the stream, handling the XDR (External Data Representation) big-endian encoding.
- **`crate::vfs::commit`**: Used to import the `commit::Args` structure. This is the target type that the `args` function constructs and returns, representing the deserialized arguments in the domain model of the Virtual File System.
- **`crate::parser`**: Used to import the `Result` type alias. This ensures that the function returns the standardized error type used throughout the parser subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments of an NFSv3 `COMMIT` procedure call from a byte stream into a structured `commit::Args` object.
- To act as the specific adapter for the `COMMIT` operation within the larger NFSv3 parsing framework, translating the wire format into the VFS interface.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded arguments for the `COMMIT` operation.

Outputs:
- `Result<commit::Args>`: A Result containing the parsed arguments (`commit::Args`) on success, or a `parser::Error` on failure.

Steps:
1. **File Handle Parsing**: The function calls `file::handle(src)` to read the file handle from the stream. This consumes the bytes corresponding to the file identifier.
2. **Offset Parsing**: The function calls `u64(src)` to read the offset (a 64-bit unsigned integer) from the stream. This indicates the starting byte position for the commit operation.
3. **Count Parsing**: The function calls `u32(src)` to read the count (a 32-bit unsigned integer) from the stream. This indicates the number of bytes to commit.
4. **Aggregation**: The function constructs an instance of `commit::Args` using the parsed `file`, `offset`, and `count` values and wraps it in `Ok`.

Edge Cases:
- **Stream Exhaustion**: If the stream ends prematurely before all fields are read, the underlying `read_exact` calls in the dependency functions will return an `IO` error, which propagates up.
- **Invalid Data**: If the file handle or integers are malformed (e.g., incorrect size or encoding), the dependency parsers will return specific errors (e.g., `BadFileHandle`, `IO`).

Complexity:
- Time: O(1). The function reads a fixed number of bytes (file handle size + 8 bytes offset + 4 bytes count).
- Space: O(1). The function allocates only the `commit::Args` struct on the stack.

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same `commit::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle`**: This function encapsulates the logic for reading a fixed-size opaque byte array (64 bytes) that represents the file handle. It ensures the handle conforms to the `NFS3_FHSIZE` constant.

- **From `nfs_mamont::parser::primitive`**:
 - **`u64` and `u32`**: These functions handle the low-level extraction of bytes and their assembly into integers, respecting the Big-Endian byte order required by the NFSv3 protocol.

- **From `nfs_mamont::vfs::commit`**:
 - **`Args`**: This struct defines the semantic meaning of the parsed fields (file, offset, count). The parser module's job is to populate this struct, effectively bridging the raw protocol data to the VFS layer's expectations.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a constructor for the `commit::Args` entity defined in `nfs_mamont::vfs::commit`.

Relations:
- **Transformation**: The `args` function transforms a byte stream (`Read`) into a `commit::Args` struct.

Global Invariants:
- **Field Order**: The function assumes the fields in the byte stream appear in the specific order: file handle, offset, then count. This order is mandated by the NFSv3 RFC 1813 specification for the `COMMIT` arguments.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type returned via the `Result` alias. Specific variants depend on which dependency fails (e.g., `Error::IO` for stream issues, `Error::BadFileHandle` for handle issues).

Error Propagation Strategy:
- **Propagation**: The module uses the `?` operator to immediately return any error encountered during the parsing of the file handle, offset, or count. It does not perform custom error handling or recovery.

Recoverability:
- **Not Recoverable for the Message**: If parsing fails, the `args` function returns an `Err`. The caller (typically the RPC dispatcher) must abandon the processing of this specific request, as the arguments are incomplete or invalid.

Panics:
- Allowed: No.
- Conditions: The code consists solely of function calls and struct construction. There are no `unwrap`, `expect`, or indexing operations that could panic.

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
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **deserialize the arguments for the NFSv3 `COMMIT` procedure**, which is the mechanism by which a client requests that the server flush cached data to stable storage. The system contains a parser subsystem that breaks down the complex NFSv3 protocol into individual procedure parsers. This module is the specific implementation for the `COMMIT` procedure.

A typical usage scenario of the system involves the RPC dispatcher receiving a request message identified as a `COMMIT` call. The dispatcher invokes the `args` function in this module, passing the network stream. The function reads the file handle (to identify *what* to commit), the offset (to identify *where* to start), and the count (to identify *how much* to commit). The resulting `commit::Args` struct is then passed to the VFS layer, which executes the actual disk synchronization.

Inside the system, the following things happen and they use this module:
- **Protocol Decoding**: The module relies on `parser::nfsv3::file` to interpret the opaque file handle and on `parser::primitive` to interpret the numeric ranges. This ensures that the byte-level details of XDR encoding (like endianness and padding) are abstracted away from the VFS logic.
- **Interface Mapping**: The module maps the raw wire data directly to the `vfs::commit::Args` structure. This structure is the input type for the `Commit` trait defined in the VFS layer. By producing this specific type, the parser module enables the rest of the server to handle the request using high-level, type-safe Rust code rather than raw bytes.

Without this module, the RPC layer would lack a standardized way to interpret `COMMIT` requests, forcing it to mix protocol parsing logic with request dispatching logic. This module encapsulates the specific knowledge of the `COMMIT` argument layout, ensuring that the server can correctly interpret client requests for data durability.