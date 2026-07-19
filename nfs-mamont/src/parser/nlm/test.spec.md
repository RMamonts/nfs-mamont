<!-- SPEC_HASH: c54539a5bcd2bdf7a8e9fa0c183e8312c572c247e81274f17ede239588aea5b1 -->
# Module Specification

Module: nfs_mamont::parser::nlm::test
Rust File: src/parser/nlm/test.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::nlm::cookie::Cookie`**: Used to wrap the raw 64-bit integer parsed from the stream into a type-safe identifier for the NLM transaction.
- **`crate::nlm::procedures::test::Nlm4TestArgs`**: Used as the target data structure that the `test` function constructs and returns. It represents the deserialized arguments for the NLMv4 `TEST` procedure.
- **`crate::parser::nlm::parse_lock`**: Used to parse the `lock` field of the arguments. This function encapsulates the logic for reading the common lock structure (caller name, file handle, owner handle, system ID, offset, length) which is shared across multiple NLM procedures.
- **`crate::parser::primitive::{bool, u64}`**: Used to read the primitive XDR-encoded data types from the byte stream. `u64` reads the cookie value, and `bool` reads the exclusive lock flag.
- **`crate::parser::Result`**: Used as the return type for the `test` function, providing a standardized error handling mechanism for the parser subsystem.
- **`std::io::Read`**: Used as the trait bound for the input source `src`, allowing the function to accept any type that provides a byte stream (e.g., `TcpStream`, `Cursor`, `&[u8]`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NLMv4 `TEST` procedure from an XDR-encoded byte stream into the `Nlm4TestArgs` structure. This allows the server to interpret the raw bytes of a network request into a structured format that the lock management logic can process.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded arguments.

Outputs:
- `Result<Nlm4TestArgs>`: A result containing the parsed `Nlm4TestArgs` structure or an error if the stream is malformed or incomplete.

Steps:
1. **Parse Cookie**: The function calls `primitive::u64(src)` to read an unsigned 64-bit integer from the stream. This value is passed to `Cookie::new` to create the `cookie` field of the arguments.
2. **Parse Exclusive Flag**: The function calls `primitive::bool(src)` to read a boolean value from the stream. This determines if the lock request being tested is exclusive (`true`) or shared (`false`).
3. **Parse Lock Structure**: The function calls `parse_lock(src)` to read the remaining fields of the arguments. This helper function reads the caller name, file handle, opaque owner handle, system ID, offset, and length, constructing an `Nlm4Lock` structure.
4. **Construct Arguments**: The function constructs the `Nlm4TestArgs` struct using the parsed `cookie`, `exclusive`, and `lock` fields.
5. **Return**: The function wraps the constructed struct in `Ok(...)` and returns it.

Edge Cases:
- **Insufficient Data**: If the stream ends before all fields can be read, the underlying primitive parsers (`u64`, `bool`, `parse_lock`) will return an `IO` error, which propagates up through the `?` operator.
- **Invalid Data**: If the boolean field is not a valid XDR boolean (0 or 1), `primitive::bool` returns an `Error::EnumDiscMismatch`. If the lock structure contains invalid strings or handles, `parse_lock` returns an appropriate error (e.g., `Error::BadFileHandle`).

Complexity:
- **Time**: O(N), where N is the total size of the variable-length fields within the `lock` structure (specifically the caller name, file handle, and opaque handle). The parsing of the `cookie` and `exclusive` fields is O(1).
- **Space**: O(N), where N is the size of the allocated strings and vectors within the `Nlm4Lock` structure contained in the result.

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `Nlm4TestArgs` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
 - **`u64(src)`**: Reads 8 bytes in Big-Endian order. Used to extract the raw cookie value.
 - **`bool(src)`**: Reads 4 bytes, validating that the value is 0 (false) or 1 (true). Used to extract the lock type flag.
- **From `nfs_mamont::parser::nlm`**:
 - **`parse_lock(src)`**: Parses the `Nlm4Lock` structure. This is the primary mechanism for extracting the complex, nested lock details (caller name, file handle, etc.) which constitute the bulk of the `TEST` arguments.
- **From `nfs_mamont::nlm::cookie`**:
 - **`Cookie::new(val)`**: A constructor that wraps a `u64` in the `Cookie` type. This mechanism ensures type safety for the transaction identifier.
- **From `nfs_mamont::nlm::procedures::test`**:
 - **`Nlm4TestArgs`**: The data structure that aggregates the parsed fields. This module acts as a factory for this type.

---

## 4. Data Model

Entities:
- This module defines no public structs or enums. It acts as a factory function for `Nlm4TestArgs`.

Relations:
- **Construction**: The `test` function constructs an instance of `Nlm4TestArgs` (defined in `crate::nlm::procedures::test`).

Global Invariants:
- The input stream `src` must contain the fields in the exact order defined by the NLMv4 XDR specification for the `TEST` procedure arguments: `cookie` (unsigned hyper), `exclusive` (bool), and `lock` (netobj, string, netobj, int, unsigned hyper, unsigned hyper).

## 5. Error Model

Error Types:
- **`crate::parser::Error`**: The error type re-exported from `crate::rpc`. This includes variants for `IO` errors (unexpected end of stream), `EnumDiscMismatch` (invalid boolean), and `BadFileHandle` (invalid lock structure).

Error Propagation Strategy:
- **Propagation**: The module uses the `?` operator to propagate errors returned by `primitive::u64`, `primitive::bool`, and `parse_lock`. It does not introduce new error types or perform custom error mapping.

Recoverability:
- **Unrecoverable for the current message**: If parsing fails, the stream cursor is likely at an undefined position relative to the message boundary. The caller (typically the RPC dispatcher) must discard the remainder of the message or close the connection.

Panics:
- Allowed: No.
- Conditions: The code relies entirely on fallible parsing functions (`u64`, `bool`, `parse_lock`) and does not use `unwrap`, `expect`, or indexing that could panic.

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

This module is used in order to deserialize the specific arguments required for the NLMv4 `TEST` procedure from the network byte stream. In the context of the `nfs_mamont` NFS server, the NLM (Network Lock Manager) protocol allows clients to query the status of a lock without acquiring it. This module serves as the specific adapter that translates the raw XDR bytes of a `TEST` request into the `Nlm4TestArgs` structure used by the server's locking logic.

The system contains a layered parsing architecture designed to handle the complexity of the ONC RPC/XDR protocols. The `primitive` module handles basic types, the `parser::nlm` module handles shared NLM structures (like the generic lock details), and this module (`parser::nlm::test`) handles the assembly of the procedure-specific arguments. A typical usage scenario involves the RPC dispatcher receiving a message identified as an NLM `TEST` call. The dispatcher invokes this `test` function, passing the network stream. The function reads the transaction cookie, the exclusive flag, and the lock details (delegating the complex lock parsing to `parse_lock`). The resulting `Nlm4TestArgs` is then passed to the `Nlm` service implementation to check for conflicts.

Inside the system, this module ensures that the specific order and data types defined by the NLMv4 RFC are strictly adhered to. By relying on `parse_lock`, it reuses the logic for reading the common lock block, ensuring consistency across `TEST`, `LOCK`, and `CANCEL` procedures. Without this module, the RPC layer would lack the specific logic to interpret `TEST` requests, preventing the server from answering client queries about lock availability.