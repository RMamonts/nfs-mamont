<!-- SPEC_HASH: b33547760fa8f4b3a15d570a4fce536bcf4ae702571c45a77ef6f8df28d42f5d -->
# Module Specification

Module: nfs_mamont::parser::nlm::lock
Rust File: src/parser/nlm/lock.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the trait bound for the input source `src`. This allows the parser to read from any source that implements the standard `Read` trait (e.g., `TcpStream`, `Cursor`, `File`).
- **`crate::nlm::cookie::Cookie`**: Used to wrap the raw `u64` cookie value read from the stream into a type-safe structure, ensuring the transaction ID is distinct from other unsigned integers in the protocol.
- **`crate::nlm::procedures::lock::Nlm4LockArgs`**: Used as the target data structure for the parsing operation. The function populates this struct with the deserialized fields to be passed to the NLM service layer.
- **`crate::parser::nlm::parse_lock`**: Used to parse the nested `lock` field within the arguments. This function handles the complex sub-structure containing the caller name, file handle, owner handle, and lock range, which is shared across multiple NLM procedures.
- **`crate::parser::primitive::{bool, u32, u64}`**: Used to read the low-level XDR-encoded primitive types from the stream. Specifically, `u64` for the cookie, `bool` for flags (`block`, `exclusive`, `reclaim`), and `u32` for the `state`.
- **`crate::parser::Result`**: Used as the return type alias, standardizing error handling across the parser modules.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NLMv4 `LOCK` procedure from an XDR-encoded byte stream into the `Nlm4LockArgs` structure. This function serves as the specific entry point for parsing lock requests within the NLM parser subsystem.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded arguments of an NLMv4 `LOCK` RPC call.

Outputs:
- `Result<Nlm4LockArgs>`: A result containing the fully populated `Nlm4LockArgs` structure if parsing succeeds, or a `parser::Error` if the stream is malformed or incomplete.

Steps:
1. **Cookie Parsing**: The function reads a `u64` from the stream using `primitive::u64` and wraps it in a `Cookie` using `Cookie::new`.
2. **Flag Parsing**: It reads three boolean flags sequentially using `primitive::bool`:
   - `block`: Indicates if the request should block.
   - `exclusive`: Indicates if the lock is exclusive.
   - `reclaim`: Indicates if the lock is a reclaim request.
3. **Lock Details Parsing**: It invokes `parse_lock(src)` to parse the nested `Nlm4Lock` structure, which contains the caller name, file handle, owner handle, system ID, offset, and length.
4. **State Parsing**: It reads a `u32` from the stream using `primitive::u32` to obtain the NSM state value.
5. **Struct Construction**: It aggregates all parsed fields into an instance of `Nlm4LockArgs` and returns it wrapped in `Ok`.

Edge Cases:
- **Insufficient Data**: If the stream ends before all fields are read, the underlying primitive parsers will return an `IO` error, which propagates up as `Err`.
- **Malformed Data**: If `parse_lock` fails (e.g., invalid caller name or oversized handle), the error is propagated immediately, halting the parsing of the remaining fields.

Complexity:
- Time: O(N), where N is the total size of the variable-length fields within the nested `lock` structure (caller name, file handle, opaque handle). The primitive fields are O(1).
- Space: O(N), where N is the size of the allocated strings and vectors within the returned `Nlm4LockArgs` (specifically inside the nested `Nlm4Lock`).

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `Nlm4LockArgs` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
 - `u64(src)`: Reads a 64-bit unsigned integer in Big-Endian format. Used to parse the cookie.
 - `bool(src)`: Reads a 32-bit integer and validates it is 0 or 1. Used to parse the `block`, `exclusive`, and `reclaim` flags.
 - `u32(src)`: Reads a 32-bit unsigned integer. Used to parse the `state` field.
- **From `nfs_mamont::parser::nlm`**:
 - `parse_lock(src)`: Parses the common lock arguments block (`Nlm4Lock`). This is crucial because it encapsulates the logic for validating the caller name and file handle, ensuring that the `LOCK` procedure parser adheres to the same validation rules as other NLM procedures (like `TEST` or `UNLOCK`).
- **From `nfs_mamont::nlm::cookie`**:
 - `Cookie::new(val)`: Constructs a type-safe cookie from a raw `u64`. This mechanism ensures that the transaction ID is treated as a distinct semantic entity within the `Nlm4LockArgs`.

---

## 4. Data Model

Entities:
- This module defines no public structs or enums. It acts as a factory for `Nlm4LockArgs` (defined in `crate::nlm::procedures::lock`).

Relations:
- None.

Global Invariants:
- The input stream must contain the fields in the exact order defined by the NLMv4 XDR specification for the `LOCK` procedure: `cookie`, `block`, `exclusive`, `lock`, `reclaim`, `state`.

## 5. Error Model

Error Types:
- **`Error`**: The error enum defined in `crate::parser` (re-exported from `crate::rpc`).

Error Propagation Strategy:
- **Custom Enum (`Result`)**: The module uses the `?` operator to propagate errors returned by the primitive parsers (`u64`, `bool`, `u32`) and the composite parser (`parse_lock`). It does not introduce new error variants or perform custom error mapping.

Recoverability:
- Unrecoverable for the current message. If an error occurs, the stream cursor is advanced to an unknown point relative to the message boundary, and the specific RPC request cannot be processed. The connection may need to be reset or the message discarded depending on the higher-level RPC framing logic.

Panics:
- Allowed: No.
- Conditions: The code relies exclusively on fallible parsing functions (`?`) and does not use `unwrap`, `expect`, or unchecked indexing.

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

This module is used in order to deserialize the specific arguments for the NLMv4 `LOCK` procedure from the network byte stream. The system contains an NFS server that implements the Network Lock Manager (NLM) protocol to coordinate file locks across clients. This module provides the specific logic required to interpret the `LOCK` operation, which is one of the core procedures in the NLM protocol.

A typical usage scenario of the system involves the RPC dispatcher receiving an NLM request identified as procedure number 2 (`LOCK`). The dispatcher delegates the parsing of the request body to this module's `lock` function. This function reads the transaction cookie (to match replies), the blocking and exclusive flags (to determine lock mode), the reclaim flag (for crash recovery), the NSM state, and the detailed lock information (file handle, owner, range) via the shared `parse_lock` function. The resulting `Nlm4LockArgs` structure is then passed to the `Nlm` service trait implementation to perform the actual lock management.

Inside the system, the following things happen and they use this module:
- **Procedure-Specific Parsing**: While the `parser::nlm` module handles common structures, this module handles the specific field ordering and types unique to the `LOCK` arguments (e.g., the presence of `reclaim` and `state` fields which might not exist or differ in other procedures).
- **Validation Delegation**: By utilizing `parse_lock`, this module ensures that the complex validation of the lock owner and file handle is not duplicated, maintaining consistency with the rest of the NLM parser subsystem.
- **Type Safety**: The use of `Cookie::new` ensures that the transaction ID is immediately wrapped in a strong type, preventing accidental confusion with other `u64` fields (like offsets) in the subsequent server logic.