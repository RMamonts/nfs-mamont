<!-- SPEC_HASH: b33547760fa8f4b3a15d570a4fce536bcf4ae702571c45a77ef6f8df28d42f5d -->
# Module Specification

Module: nfs_mamont::parser::nlm::lock
Rust File: src/parser/nlm/lock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`std::io::Read`**:
    *   Used as the source trait for the input byte stream (`src`). This allows the parser to work with TCP streams, file buffers, or test vectors (`Cursor`).
*   **`crate::nlm::cookie::Cookie`**:
    *   Used to wrap the raw `u64` cookie value read from the stream into a type-safe identifier required by `Nlm4LockArgs`.
*   **`crate::nlm::procedures::lock::Nlm4LockArgs`**:
    *   Used as the target data structure that the function populates and returns. This struct represents the high-level, domain-specific arguments for the NLM LOCK procedure.
*   **`crate::parser::nlm::parse_lock`**:
    *   Used to parse the nested `lock` field (of type `Nlm4Lock`) within the arguments. This field contains complex data like the file handle, owner ID, and range.
*   **`crate::parser::primitive::{bool, u32, u64}`**:
    *   Used to read primitive XDR-encoded data types from the stream. These functions handle byte order (Big-Endian) and alignment.
*   **`crate::parser::Result`**:
    *   Used as the return type for the parsing function, encapsulating either the successfully parsed `Nlm4LockArgs` or an error encountered during reading.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module.
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments of an NLMv4 `LOCK` procedure call from a byte stream conforming to the XDR (External Data Representation) standard. This function acts as an adapter between the raw network protocol format and the internal Rust representation used by the NLM service logic.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream (e.g., network socket or buffer) containing the XDR-encoded arguments.

Outputs:
- `Result<Nlm4LockArgs>`: A Result containing the parsed `Nlm4LockArgs` structure if successful, or an error if the stream is invalid, incomplete, or contains malformed data.

Steps:
1.  **Cookie Parsing**: Read a `u64` from the stream using `primitive::u64`. Wrap this value in `Cookie::new` to create the `cookie` field.
2.  **Flag Parsing**: Read a boolean value using `primitive::bool` to populate the `block` field.
3.  **Exclusive Parsing**: Read another boolean value using `primitive::bool` to populate the `exclusive` field.
4.  **Lock Details Parsing**: Invoke `parse_lock(src)` to parse the nested `Nlm4Lock` structure, which includes the caller name, file handle, offset, and length. Assign this to the `lock` field.
5.  **Reclaim Parsing**: Read a boolean value using `primitive::bool` to populate the `reclaim` field.
6.  **State Parsing**: Read a `u32` from the stream using `primitive::u32` to populate the `state` field.
7.  **Construction**: Aggregate all parsed fields into an instance of `Nlm4LockArgs` and return it wrapped in `Ok`.

Edge Cases:
- **Insufficient Data**: If the `src` stream ends before all fields are read, the underlying `read_exact` calls in the primitive parsers will return an I/O error, which propagates up as a `Result::Err`.
- **Invalid Encoding**: If the boolean fields contain values other than 0 or 1 (as defined by XDR), the `primitive::bool` function will return an `EnumDiscMismatch` error.

Complexity:
- Time: O(N), where N is the size of the `Nlm4Lock` field (specifically the length of variable-length fields like the caller name or opaque handle). The parsing of the outer fields (`cookie`, `block`, etc.) is O(1).
- Space: O(N), where N is the size of the `Nlm4Lock` structure, as it allocates memory for strings and byte vectors within that struct.

Determinism:
- Deterministic. Given a specific byte sequence, the function will always produce the same `Nlm4LockArgs` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
    - **`u64(src)`**: Reads 8 bytes in Big-Endian order. Used to parse the `cookie` and potentially parts of the `lock` structure.
    - **`bool(src)`**: Reads 4 bytes (XDR standard for booleans) and validates that the value is 0 (false) or 1 (true). Used to parse `block`, `exclusive`, and `reclaim`.
    - **`u32(src)`**: Reads 4 bytes in Big-Endian order. Used to parse the `state` field.
- **From `nfs_mamont::nlm::cookie`**:
    - **`Cookie::new(val)`**: Constructs a type-safe wrapper around the raw `u64` cookie value read from the wire.
- **From `nfs_mamont::parser::nlm` (Assumption based on facts)**:
    - **`parse_lock(src)`**: A specialized parser for the `Nlm4Lock` structure. It handles the specific XDR layout of the lock holder information (caller name, system ID, file handle, offset, length).

---

## 4. Data Model

Entities:
- This module does not define any public entities (structs or enums). It acts as a factory for the `Nlm4LockArgs` entity defined in `crate::nlm::procedures::lock`.

Relations:
- N/A (This module is a pure function module).

Global Invariants:
- The order of fields in the byte stream must strictly follow the NLMv4 XDR definition: `cookie`, `block`, `exclusive`, `lock`, `reclaim`, `state`.
- The `cookie` value returned in `Nlm4LockArgs` must be identical to the `u64` value read from the start of the stream.

---

## 5. Error Model

Error Types:
- **`Error`**: The error type defined in `crate::parser` (likely re-exported from `crate::rpc` or defined locally). Specific variants expected include:
    - `Error::IO`: Indicates the stream ended prematurely or a read failure occurred.
    - `Error::EnumDiscMismatch`: Indicates a boolean field contained a value other than 0 or 1.

Error Propagation Strategy:
- **Custom Enum (`Error`)**: The function uses the `?` operator to propagate errors returned by `primitive::u64`, `primitive::bool`, `primitive::u32`, and `parse_lock`. It does not introduce new error types.

Recoverability:
- **Unrecoverable**: If parsing fails, the stream is left in an undefined state (partially consumed). The caller must discard the rest of the message or reset the connection.

Panics:
- Allowed: No.
- Conditions: The code consists entirely of calls to fallible parsing functions and struct construction. No `unwrap`, `expect`, or index access is used that could cause a panic.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding.
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module.
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed.
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module.
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level.
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to deserialize the Network Lock Manager (NLM) version 4 `LOCK` procedure arguments from the network wire format (XDR) into a structured Rust representation. In the broader system, this serves as the critical bridge between the RPC transport layer—which deals only with raw byte streams—and the NLM service logic—which requires semantic objects to make decisions about file locking.

This system contains a specialized parser that understands the specific layout of the NLM `LOCK` arguments as defined in RFC 1813. It relies on the `primitive` module to handle the low-level details of XDR (endianness, alignment, padding) and the `parse_lock` helper to handle the complex nested structure describing the file handle and lock range. By isolating this logic, the system ensures that the RPC handler remains generic and that the NLM service implementation does not need to concern itself with byte-level parsing.

A typical usage scenario of the system involves an NFS server receiving an NLM `LOCK` request over the network. The RPC dispatcher reads the request bytes and invokes this `lock` function. The function consumes the stream, extracting the `cookie` (for transaction tracking), the `block` flag (to determine if the client wants to wait), the `exclusive` flag (for lock type), and the `lock` details (file handle, owner, range). The resulting `Nlm4LockArgs` is then passed to the `Nlm` service trait implementation, which inspects the VFS and internal lock state to grant or deny the request.

Inside the system the following things happen and they use the `Cookie` type to ensure the transaction ID is handled as a distinct entity, and the `primitive` parsers to strictly enforce the XDR standard, preventing malformed or malicious network data from corrupting the server's memory or state.