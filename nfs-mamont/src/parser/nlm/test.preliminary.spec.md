<!-- SPEC_HASH: c54539a5bcd2bdf7a8e9fa0c183e8312c572c247e81274f17ede239588aea5b1 -->
# Module Specification

Module: nfs_mamont::parser::nlm::test
Rust File: src/parser/nlm/test.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **Standard Library (`std::io::Read`)**:
    *   Used as a trait bound for the input source `src`. It allows the parser to read bytes from various sources (network streams, buffers, files) generically.
*   **`crate::nlm::cookie::Cookie`**:
    *   Used to wrap the raw `u64` transaction identifier read from the stream. This provides type safety for the cookie value within the `Nlm4TestArgs` structure.
*   **`crate::nlm::procedures::test::Nlm4TestArgs`**:
    *   Used as the return type. This structure represents the high-level, deserialized arguments required for the NLMv4 `TEST` procedure.
*   **`crate::parser::nlm::parse_lock`**:
    *   Used to parse the `lock` field of the arguments. This function handles the deserialization of the complex `Nlm4Lock` structure (which includes caller name, file handle, offsets, etc.) from the stream.
*   **`crate::parser::primitive::{bool, u64}`**:
    *   Used to read the primitive XDR data types from the stream. `u64` reads the cookie value, and `bool` reads the exclusive lock flag.
*   **`crate::parser::Result`**:
    *   Used as the return type wrapper. It propagates errors that may occur during reading or parsing (e.g., unexpected EOF, invalid data).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the byte stream representation of an NLMv4 `TEST` procedure call into the structured `Nlm4TestArgs` Rust type. This function acts as the specific parser for this RPC procedure, ensuring the binary data conforms to the expected XDR layout.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream. The stream position is advanced as data is consumed.

Outputs:
- `Result<Nlm4TestArgs>`: A Result containing the parsed arguments if successful, or an error if the stream is malformed or incomplete.

Steps:
1.  **Parse Cookie**: Invoke `u64(src)` to read 8 bytes (Big-Endian) representing the transaction identifier. Wrap this value in `Cookie::new`.
2.  **Parse Exclusive Flag**: Invoke `bool(src)` to read 4 bytes (XDR boolean) representing whether the lock is exclusive (`true`) or shared (`false`).
3.  **Parse Lock Details**: Invoke `parse_lock(src)` to read the remaining bytes corresponding to the `Nlm4Lock` structure. This includes the caller name, file handle, system ID, and lock range.
4.  **Construct Arguments**: Aggregate the parsed `cookie`, `exclusive`, and `lock` fields into the `Nlm4TestArgs` struct and return it wrapped in `Ok`.

Edge Cases:
- **Insufficient Data**: If the `src` stream ends before all fields (or sub-fields within `parse_lock`) can be read, the underlying primitive parsers will return an `Error`, which is propagated up.
- **Invalid Data**: If the boolean value is not 0 or 1, or if the `Nlm4Lock` data is invalid (e.g., bad UTF-8 in the caller name), an error is returned.

Complexity:
- Time: O(N), where N is the size of the `Nlm4Lock` structure in bytes (since `cookie` and `exclusive` are constant size). The complexity is dominated by the `parse_lock` call.
- Space: O(N), where N is the size of the `Nlm4Lock` structure, due to heap allocations for strings and vectors within it.

Determinism:
- Deterministic. Given a valid byte stream, the function always produces the same `Nlm4TestArgs` structure.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **`nfs_mamont::parser::primitive`**:
    - `u64(src)`: Reads a 64-bit unsigned integer in Big-Endian format.
    - `bool(src)`: Reads a 32-bit integer and interprets 0 as `false` and 1 as `true`.
- **`nfs_mamont::nlm::cookie`**:
    - `Cookie::new(val)`: Constructs a new `Cookie` instance from a `u64` value.
- **`nfs_mamont::parser::nlm`**:
    - `parse_lock(src)`: Parses a `Nlm4Lock` structure, which involves reading strings, opaque handles, and integers, handling XDR alignment and padding internally.

---

## 4. Data Model

Entities:
- None defined in this module. The module constructs an instance of `Nlm4TestArgs` defined in `crate::nlm::procedures::test`.

Relations:
- The function establishes a temporary composition relation between the parsed primitives (`Cookie`, `bool`, `Nlm4Lock`) to form the `Nlm4TestArgs` aggregate.

Global Invariants:
- The input stream must strictly follow the XDR encoding order: `cookie` (unsigned hyper), `exclusive` (boolean), `lock` (struct).
- The stream cursor is advanced. If the function returns `Ok`, the cursor points to the byte immediately following the `TEST` arguments. If it returns `Err`, the cursor position is undefined (likely at the point of failure).

## 5. Error Model

Error Types:
- `crate::parser::Error` (re-exported from `crate::rpc::Error`).

Error Propagation Strategy:
- Propagation via the `?` operator. Errors from `u64`, `bool`, and `parse_lock` are returned immediately to the caller.

Recoverability:
- Unrecoverable for the current parsing attempt. The caller must handle the error (e.g., by dropping the connection or sending an RPC rejection message).

Panics:
- Allowed: No.
- Conditions: The code relies on `?` for error handling and does not perform any operations that could panic (like unwrapping `None` or indexing out of bounds).

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

This module is used in order to deserialize the arguments of the Network Lock Manager (NLM) version 4 `TEST` procedure from the XDR (External Data Representation) format used in ONC RPC. In the NLM protocol, the `TEST` procedure allows a client to check if a lock request would be granted without actually acquiring it. This module serves as the adapter that converts the raw byte stream received from the network into a structured Rust representation (`Nlm4TestArgs`) that the server logic can process.

This system contains the specific parsing logic required to interpret the `TEST` procedure's unique layout of fields. It relies on the `primitive` module to handle the low-level byte reading and XDR alignment rules, and the `nlm::cookie` module to enforce type safety on the transaction identifier. By delegating the parsing of the complex `lock` field to `parse_lock`, it reuses the logic for reading lock details which is common across other NLM procedures (like `LOCK` and `UNLOCK`).

A typical usage scenario of the system involves the RPC layer receiving a request message identified as procedure 1 (`TEST`) for program 100021 (`NLM`). The RPC dispatcher calls this `test` function, passing the payload portion of the message. The function reads the cookie to identify the transaction, the exclusive flag to determine the lock type, and the lock details to identify the file and range. The resulting `Nlm4TestArgs` is then passed to the handler implementing the `Test` trait to perform the actual lock check.

Inside the system the following things happen and they use the `test` function to bridge the gap between the physical network layer (bytes) and the logical application layer (structs). It ensures that the data adheres to the protocol specification before the server attempts to act on it, preventing malformed packets from affecting the lock manager's state.