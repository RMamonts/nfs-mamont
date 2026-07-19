<!-- SPEC_HASH: e730fd58eb68452a1a616b64ecf3bd8ac7422cb3eff64e0739529768bda67076 -->
# Module Specification

Module: nfs_mamont::parser::nlm::cancel
Rust File: src/parser/nlm/cancel.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::cookie::Cookie`**:
    *   Used to wrap the raw 64-bit integer parsed from the stream into a type-safe `Cookie` struct, which serves as the transaction identifier for the NLM procedure.
*   **`crate::nlm::procedures::cancel::Nlm4CancelArgs`**:
    *   Used as the return type of the parsing function. It is the target data structure that aggregates the parsed fields (cookie, block flag, exclusive flag, and lock details) required by the NLM service layer.
*   **`crate::parser::nlm::parse_lock`**:
    *   Used to parse the `lock` field within the `CANCEL` arguments. Since the lock definition (caller name, file handle, owner, range) is complex and shared across multiple NLM procedures, this function delegates the parsing of that sub-structure to the parent NLM parser module.
*   **`crate::parser::primitive::{bool, u64}`**:
    *   Used to read the primitive XDR-encoded data types from the input stream. `u64` reads the cookie value, and `bool` reads the `block` and `exclusive` flags.
*   **`crate::parser::Result`**:
    *   Used as the return type for the parsing function, allowing the propagation of I/O errors or protocol validation errors (e.g., invalid boolean values) to the caller.
*   **`std::io::Read`**:
    *   Used as the trait bound for the input source (`src`), allowing the parser to read bytes from any source that implements the standard `Read` trait (e.g., TCP streams, memory buffers).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the byte stream representation of an NLMv4 `CANCEL` procedure call into the `Nlm4CancelArgs` structure, validating the format and extracting the specific parameters needed to identify and cancel a pending lock request.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded arguments of the `CANCEL` procedure.

Outputs:
- `Result<Nlm4CancelArgs>`: A structure containing the parsed `cookie`, `block` flag, `exclusive` flag, and `lock` details, or an error if the stream is invalid or incomplete.

Steps:
1.  **Cookie Parsing**: The function calls `u64(src)` to read a 64-bit unsigned integer from the stream. This value is passed to `Cookie::new` to create a `Cookie` instance, which is assigned to the `cookie` field of `Nlm4CancelArgs`.
2.  **Flag Parsing**: The function calls `bool(src)` twice to read the `block` and `exclusive` boolean flags. These flags indicate whether the original request was blocking and whether it was for an exclusive lock.
3.  **Lock Parsing**: The function calls `parse_lock(src)` to parse the remaining bytes of the stream into an `Nlm4Lock` structure. This structure contains the detailed information required to identify the specific lock to cancel (caller name, file handle, owner, offset, length).
4.  **Struct Construction**: The function aggregates the parsed `cookie`, `block`, `exclusive`, and `lock` fields into the `Nlm4CancelArgs` struct and wraps it in `Ok` to return success.

Edge Cases:
- **Insufficient Data**: If the input stream contains fewer bytes than required for the `u64`, booleans, or the `Nlm4Lock` structure, the underlying primitive parsers or `parse_lock` will return an `IO` error, which propagates immediately.
- **Invalid Boolean**: If the stream contains a value other than 0 or 1 for the boolean fields, the `primitive::bool` function returns an `Error::EnumDiscMismatch`, causing the parse to fail.

Complexity:
- **Time**: O(N), where N is the total size of the variable-length fields within the `Nlm4Lock` structure (e.g., caller name, file handle). The parsing of the fixed-size primitives (cookie, flags) is O(1).
- **Space**: O(N), where N is the size of the allocated strings and vectors inside the `Nlm4Lock` structure.

Determinism:
- Deterministic. Given the same sequence of bytes in the input stream, the function will always produce the same `Nlm4CancelArgs` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
    - **`u64(src)`**: Reads a Big-Endian unsigned 64-bit integer. This is used to extract the raw cookie value which acts as the transaction ID.
    - **`bool(src)`**: Reads a 32-bit integer and strictly validates it as 0 (false) or 1 (true). This ensures the `block` and `exclusive` flags conform to the XDR standard for booleans.

- **From `nfs_mamont::parser::nlm`**:
    - **`parse_lock(src)`**: Parses the `Nlm4Lock` structure. This is a critical dependency because the `CANCEL` arguments must contain the exact lock definition (caller, file, range) that the client wishes to cancel. This function handles the complexity of parsing the nested string and opaque data fields within the lock structure.

- **From `nfs_mamont::nlm::cookie`**:
    - **`Cookie::new(val)`**: A constructor that wraps a `u64` in a `Cookie` struct. This provides type safety, distinguishing the transaction ID from other `u64` values in the system.

---

## 4. Data Model

Entities:
- This module defines no public structs or enums. It acts as a factory for the `Nlm4CancelArgs` struct defined in `crate::nlm::procedures::cancel`.

Relations:
- N/A.

Global Invariants:
- The input stream must strictly follow the XDR encoding order for `Nlm4CancelArgs`: `cookie` (u64), `block` (bool), `exclusive` (bool), and `lock` (Nlm4Lock).

## 5. Error Model

Error Types:
- **`crate::parser::Error`**: The error type defined in the parent parser module (re-exported from `crate::rpc`).

Error Propagation Strategy:
- **Propagation via `?`**: The function uses the `?` operator to propagate errors returned by `u64`, `bool`, and `parse_lock`. This means any I/O error (e.g., unexpected end of stream) or validation error (e.g., invalid boolean discriminant) stops the parsing process immediately and returns the error to the caller.

Recoverability:
- **Unrecoverable for the current message**: If parsing fails, the stream cursor is left at an undefined position relative to the message boundary. The caller cannot simply retry parsing the same stream without resetting it to a known valid state (e.g., the start of the next RPC message).

Panics:
- Allowed: No.
- Conditions: The code consists entirely of fallible parsing calls and struct construction. There are no `unwrap`, `expect`, or indexing operations that could cause a panic.

---

## 6. Traits

List which external traits this module implements:
- None.

Traits defined by this module:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to deserialize the specific arguments required for the NLMv4 `CANCEL` procedure from the network byte stream. In the context of a Network Lock Manager, the `CANCEL` procedure allows a client to retract a pending lock request that is currently blocked (waiting for a conflicting lock to be released). This module is responsible for interpreting the raw XDR payload sent by the client to ensure the server receives the correct transaction ID and the precise definition of the lock to be canceled.

The system contains a layered parsing architecture where low-level primitive parsers (`primitive::u64`, `primitive::bool`) handle the basic data types, and mid-level parsers (`parser::nlm::parse_lock`) handle complex, reusable structures. This module sits at the high-level protocol procedure layer, orchestrating these lower-level parsers to construct the complete `Nlm4CancelArgs` structure.

A typical usage scenario of the system involves the RPC dispatcher receiving a request identified as the NLM `CANCEL` procedure. The dispatcher invokes this `cancel` function, passing the network stream. The function reads the `cookie` to match the request to a previous transaction, reads the `block` and `exclusive` flags to confirm the nature of the original request, and reads the `lock` details to identify exactly which lock entry should be removed from the server's wait queue.

Inside the system, the following things happen and they use this module:
- **Protocol Compliance**: By delegating to `parse_lock`, the module ensures that the lock details (caller name, file handle, etc.) are parsed exactly as they are in other NLM procedures (like `LOCK` or `TEST`), maintaining consistency across the protocol implementation.
- **Type Safety**: The conversion of the raw `u64` cookie into a `Cookie` struct ensures that the transaction ID is treated as a distinct semantic entity, preventing accidental confusion with file offsets or other numeric values.
- **Error Handling**: The module relies on the `Result` type to immediately surface any issues with the incoming data (e.g., malformed booleans or truncated data) to the RPC layer, allowing the server to reject invalid requests before they reach the core locking logic.