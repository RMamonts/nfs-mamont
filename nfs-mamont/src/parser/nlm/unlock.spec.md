<!-- SPEC_HASH: 4b96d22cb1d778b664fe1237a0b561d92c55f3bd50d8046d6c718dde2682b8f5 -->
# Module Specification

Module: nfs_mamont::parser::nlm::unlock
Rust File: src/parser/nlm/unlock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::cookie::Cookie`**:
    *   Used to wrap the raw 64-bit integer read from the stream into a type-safe structure representing the transaction cookie.
*   **`crate::nlm::procedures::unlock::Nlm4UnlockArgs`**:
    *   Used as the return type of the parsing function. This structure aggregates the parsed `cookie` and `lock` data required for the NLMv4 `UNLOCK` procedure.
*   **`crate::parser::nlm::parse_lock`**:
    *   Used to parse the `lock` field of the arguments. This function handles the complex deserialization of the `Nlm4Lock` structure (caller name, file handle, owner, offset, length), which is shared across multiple NLM procedures.
*   **`crate::parser::primitive::u64`**:
    *   Used to read the raw 64-bit unsigned integer representing the cookie from the input stream in Big-Endian format.
*   **`crate::parser::Result`**:
    *   Used as the return type for the parsing function, standardizing error handling across the parser crate.
*   **`std::io::Read`**:
    *   Used as the trait bound for the input source `src`, allowing the parser to read bytes from any source that implements the `Read` trait (e.g., `TcpStream`, `Cursor`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NLMv4 `UNLOCK` procedure from an XDR-encoded byte stream into the `Nlm4UnlockArgs` structure. This function acts as the specific adapter that maps the wire format for the `UNLOCK` call to the internal Rust representation.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded arguments.

Outputs:
- `Result<Nlm4UnlockArgs>`: A result containing the parsed arguments structure or an error if the stream is invalid or incomplete.

Steps:
1.  **Cookie Extraction**: The function calls `primitive::u64(src)` to read the next 8 bytes from the stream as a Big-Endian unsigned 64-bit integer.
2.  **Cookie Construction**: The raw integer value is passed to `Cookie::new` to create a `Cookie` instance.
3.  **Lock Parsing**: The function calls `parse_lock(src)` to parse the subsequent bytes. This reads the caller name, file handle, opaque owner handle, system ID, offset, and length.
4.  **Aggregation**: The function constructs the `Nlm4UnlockArgs` struct using the created `cookie` and the parsed `lock` structure.
5.  **Return**: The populated `Nlm4UnlockArgs` is wrapped in `Ok` and returned.

Edge Cases:
- **Insufficient Data**: If the stream ends before the cookie or the full lock structure can be read, the underlying `u64` or `parse_lock` functions will return an `IO` error, which propagates immediately.
- **Invalid Lock Data**: If the data within the lock structure is malformed (e.g., invalid UTF-8, oversized handles), `parse_lock` will return a specific parsing error (e.g., `BadFileHandle`), which propagates.

Complexity:
- **Time**: O(N), where N is the total size of the variable-length fields within the `lock` structure (caller name, file handle, opaque handle). Reading the cookie is O(1).
- **Space**: O(N), where N is the size of the allocated strings and vectors within the `Nlm4Lock` structure.

Determinism:
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
    - **`u64(src)`**: Reads a 64-bit integer in Big-Endian format. This is used to extract the transaction cookie which must be echoed back in the response.
- **From `nfs_mamont::parser::nlm`**:
    - **`parse_lock(src)`**: Parses the common `Nlm4Lock` structure. This mechanism is reused here because the `UNLOCK` operation requires the full specification of the lock to be released (identical to the `LOCK` or `TEST` arguments), ensuring consistency in how lock identities are interpreted across different procedures.
- **From `nfs_mamont::nlm::cookie`**:
    - **`Cookie::new(val)`**: Wraps the raw integer. This provides type safety, ensuring the transaction ID is treated distinctly from other numeric values in the protocol.

---

## 4. Data Model

Entities:
- None defined in this module. The module operates on `Nlm4UnlockArgs` (defined in `crate::nlm::procedures::unlock`) and `Cookie` (defined in `crate::nlm::cookie`).

Relations:
- **Composition**: The `unlock` function constructs a `Nlm4UnlockArgs` entity by composing a `Cookie` entity and an `Nlm4Lock` entity.

Global Invariants:
- The input stream must contain the cookie field followed immediately by the lock fields in the exact order specified by the NLMv4 XDR definition.

## 5. Error Model

Error Types:
- **`crate::parser::Error`**: The error type aliased as `Result`. This encompasses IO errors (stream ended unexpectedly) and validation errors (malformed data).

Error Propagation Strategy:
- **Propagation**: The module uses the `?` operator to propagate errors returned by `primitive::u64` and `parse_lock`. It does not introduce new error variants or perform custom error mapping.

Recoverability:
- **Unrecoverable for the Message**: If parsing fails, the stream cursor is left at an undefined position relative to the message boundary. The RPC message cannot be processed, and the connection layer must typically discard the remainder of the message or close the connection.

Panics:
- Allowed: No.
- Conditions: The code consists entirely of fallible parsing calls (`?`) and struct construction. No `unwrap`, `expect`, or indexing operations that could panic are present.

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

This module is used in order to perform the deserialization of the Network Lock Manager (NLM) version 4 `UNLOCK` procedure arguments from the network wire format. In the broader system, RPC requests arrive as raw byte streams. This module serves as the specific interpreter for the `UNLOCK` operation, translating those bytes into the `Nlm4UnlockArgs` structure that the server's locking logic understands.

The system contains a layered parsing architecture where low-level primitives handle byte alignment and endianness, mid-level modules handle complex structures (like the generic `Nlm4Lock`), and high-level modules (like this one) assemble procedure-specific arguments. This module is necessary because the `UNLOCK` procedure has a specific argument layout (a `cookie` followed by a `lock` block) that differs slightly from other procedures or requires specific handling to match the Rust struct definition.

A typical usage scenario of the system involves the RPC dispatcher receiving a request identified as `NLM4_UNLOCK`. The dispatcher calls this `unlock` function, passing the incoming stream. The function reads the transaction cookie (essential for matching the asynchronous response) and the lock details (essential for identifying *which* lock to release). The resulting `Nlm4UnlockArgs` is then passed to the `Unlock` trait implementation.

Inside the system, the following things happen and they use this module:
- **Transaction Correlation**: By parsing the `cookie` field immediately, the module ensures that the unique identifier for this RPC call is available to the service layer, which must copy it into the response.
- **Lock Identification**: By delegating the parsing of the `lock` field to `parse_lock`, the module ensures that the complex logic for validating file handles and owner identifiers is reused exactly as it is for `LOCK` or `TEST` procedures, maintaining consistency in how locks are addressed across the protocol.

Without this module, the RPC layer would lack a dedicated function to convert `UNLOCK` bytes into the domain objects required by the `Nlm` service, breaking the chain between the network transport and the file locking logic.