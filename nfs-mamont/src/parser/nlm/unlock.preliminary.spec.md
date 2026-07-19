<!-- SPEC_HASH: 4b96d22cb1d778b664fe1237a0b561d92c55f3bd50d8046d6c718dde2682b8f5 -->
# Module Specification

Module: nfs_mamont::parser::nlm::unlock
Rust File: src/parser/nlm/unlock.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`std::io::Read`**: Used as the source trait for the input byte stream (`src`). This allows the parser to work with any source that provides bytes (TCP streams, files, in-memory buffers).
*   **`crate::nlm::cookie::Cookie`**: Used to wrap the raw `u64` cookie value parsed from the stream into a type-safe identifier. This ensures that the transaction ID is distinct from other `u64` values in the system.
*   **`crate::nlm::procedures::unlock::Nlm4UnlockArgs`**: Used as the return type. This structure aggregates the parsed `cookie` and `lock` data into the high-level representation of an NLM `UNLOCK` request.
*   **`crate::parser::primitive::u64`**: Used to read the raw 8-byte unsigned integer representing the cookie from the input stream according to XDR (Big-Endian) rules.
*   **`crate::parser::nlm::parse_lock`**: Used to parse the `Nlm4Lock` structure from the stream.
    *   *Assumption*: Based on the test data in this module and the public interface of `crate::parser::nlm`, `parse_lock` consumes the following fields in order: `caller_name` (string), `system_identifier` (handle), `file_handle` (opaque), `owner` (opaque), `svid` (i32), `offset` (u64), and `length` (u64).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NLMv4 `UNLOCK` procedure from a byte stream conforming to the XDR (External Data Representation) standard. This function acts as an adapter between the raw network protocol layer and the internal Rust type system.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream (e.g., a network socket or buffer cursor) containing the serialized `Nlm4UnlockArgs`.

Outputs:
- `Result<Nlm4UnlockArgs>`: A Result containing the parsed arguments if successful, or a parsing error if the stream is malformed or incomplete.

Steps:
1.  **Cookie Parsing**: The function calls `crate::parser::primitive::u64(src)` to read 8 bytes from the stream. These bytes are interpreted as a Big-Endian unsigned integer.
2.  **Cookie Construction**: The parsed integer is passed to `Cookie::new` to create a type-safe `Cookie` instance.
3.  **Lock Parsing**: The function calls `crate::parser::nlm::parse_lock(src)` to consume the subsequent bytes representing the lock details (caller name, file handle, offset, length, etc.).
4.  **Aggregation**: The `cookie` and the parsed `lock` are combined into the `Nlm4UnlockArgs` struct, which is then wrapped in `Ok` and returned.

Edge Cases:
- **Insufficient Data**: If the stream ends before the required bytes for the cookie or the lock structure are read, the underlying `u64` or `parse_lock` functions will return an `Error` (likely `IO` or `UnexpectedEof`), which is propagated up via the `?` operator.
- **Invalid Data**: If the data in the stream does not conform to XDR rules (e.g., invalid UTF-8 for the caller name inside `parse_lock`), an error is returned.

Complexity:
- Time: O(1) regarding control flow, though dependent on the size of variable-length fields (strings, opaque handles) within `parse_lock`.
- Space: O(1) for the parser logic itself, plus the memory allocated for the `Nlm4UnlockArgs` struct (which includes heap allocations for strings/vectors inside `Nlm4Lock`).

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `Nlm4UnlockArgs` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **`primitive::u64` (from `nfs_mamont::parser::primitive`)**: Provides the mechanism to read a 64-bit unsigned integer in Big-Endian format. This is critical for reading the initial cookie field.
- **`Cookie::new` (from `nfs_mamont::nlm::cookie`)**: Provides the mechanism to wrap the raw integer into a strongly-typed `Cookie` struct, enforcing type safety.
- **`parse_lock` (from `nfs_mamont::parser::nlm`)**: Provides the mechanism to parse the complex `Nlm4Lock` structure. This function handles the sequential parsing of multiple fields (strings, handles, integers) that constitute the lock definition.

---

## 4. Data Model

Entities:
- None defined in this module. The module operates on `Nlm4UnlockArgs` (defined in `crate::nlm::procedures::unlock`) and `Cookie` (defined in `crate::nlm::cookie`).

Relations:
- The module constructs a `Nlm4UnlockArgs` entity which has a composition relationship with `Cookie` and `Nlm4Lock`.

Global Invariants:
- **Field Order**: The byte stream must contain the `cookie` field strictly before the `lock` fields. This order is mandated by the NLMv4 XDR definition.
- **XDR Compliance**: The input stream must adhere to XDR encoding rules (Big-Endian integers, 4-byte alignment for variable-length data) for the parsing to succeed.

## 5. Error Model

Error Types:
- **`Error`**: The error type defined in the parent `parser` module (likely `crate::parser::Error` or `crate::rpc::Error`).

Error Propagation Strategy:
- **Propagation**: The function uses the `?` operator to propagate errors returned by `u64(src)` and `parse_lock(src)`. It does not define its own error variants.

Recoverability:
- **Unrecoverable**: If this function returns an `Err`, the stream is likely in an inconsistent state (partially read), and the current RPC request cannot be processed. The caller should typically discard the request or close the connection.

Panics:
- Allowed: No.
- Conditions: The code consists of function calls and struct construction. There are no explicit `panic!`, `unwrap()`, or `expect()` calls in the main logic (tests use `unwrap`).

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to deserialize the specific payload of an NLM (Network Lock Manager) version 4 `UNLOCK` procedure call received over the network. In the broader context of an NFS server, the system receives raw bytes representing RPC calls. This module serves as the specialized decoder that translates those bytes into the structured arguments (`Nlm4UnlockArgs`) required by the lock management logic.

This system contains a parser implementation that bridges the generic XDR parsing primitives (which handle byte alignment and integer encoding) and the domain-specific NLM data structures (which define the semantics of locks and cookies). It ensures that the raw binary data is correctly interpreted as a transaction ID (`Cookie`) and a lock descriptor (`Nlm4Lock`).

A typical usage scenario of the system involves the server receiving an RPC request identified as an NLM `UNLOCK` procedure. The server invokes this `unlock` function, passing the network stream. The function reads the cookie to identify the transaction, then reads the lock details to identify *which* lock to release. The resulting `Nlm4UnlockArgs` is then passed to the asynchronous `Unlock` trait implementation (defined in `nlm::procedures::unlock`) to perform the actual state change in the lock manager.

Inside the system the following things happen and they use the `primitive::u64` function to extract the transaction cookie, ensuring it is read as a 64-bit integer, and `parse_lock` to handle the complex nested structure of the lock arguments, ensuring that all fields (caller name, file handle, offsets) are extracted in the correct order defined by the protocol.