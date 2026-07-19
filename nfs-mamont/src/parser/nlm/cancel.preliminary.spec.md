<!-- SPEC_HASH: e730fd58eb68452a1a616b64ecf3bd8ac7422cb3eff64e0739529768bda67076 -->
# Module Specification

Module: nfs_mamont::parser::nlm::cancel
Rust File: src/parser/nlm/cancel.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::cookie::Cookie`**:
    *   Used to wrap the raw `u64` value read from the stream into a type-safe identifier representing the transaction cookie.
*   **`crate::nlm::procedures::cancel::Nlm4CancelArgs`**:
    *   Used as the target structure to be populated and returned, representing the deserialized arguments of the NLM CANCEL procedure.
*   **`crate::parser::nlm::parse_lock`**:
    *   Used to parse the nested `Nlm4Lock` structure (containing caller name, file handle, owner, offset, and length) from the byte stream.
*   **`crate::parser::primitive::{bool, u64}`**:
    *   Used to read the primitive XDR-encoded fields from the input stream: `u64` for the cookie value, and `bool` for the `block` and `exclusive` flags.
*   **`std::io::Read`**:
    *   Used as a trait bound for the `src` parameter, allowing the parser to read bytes from any source (e.g., network stream or memory buffer).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module.
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the byte stream representing an NLMv4 `CANCEL` procedure call into the `Nlm4CancelArgs` Rust structure, adhering to the XDR (External Data Representation) encoding standard.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded arguments.

Outputs:
- `Result<Nlm4CancelArgs>`: A Result containing the parsed `Nlm4CancelArgs` structure or an error if parsing fails.

Steps:
1.  **Parse Cookie**: Invoke `u64(src)` to read 8 bytes (Big-Endian) representing the cookie value. Wrap this value in `Cookie::new`.
2.  **Parse Block Flag**: Invoke `bool(src)` to read 4 bytes (XDR encoded boolean) representing the `block` field.
3.  **Parse Exclusive Flag**: Invoke `bool(src)` to read 4 bytes representing the `exclusive` field.
4.  **Parse Lock Details**: Invoke `parse_lock(src)` to read the remaining bytes, which constitute the `Nlm4Lock` structure (caller name, file handle, lock owner, offset, length).
5.  **Construct Arguments**: Aggregate the parsed fields into `Nlm4CancelArgs` and return it wrapped in `Ok`.

Edge Cases:
- **Insufficient Data**: If the stream ends before all fields can be read, the underlying `read_exact` calls will fail, propagating an `Error::IO`.
- **Invalid Encoding**: If the boolean fields contain values other than 0 or 1, the `bool` parser will return an `Error::EnumDiscMismatch`.

Complexity:
- Time: O(N), where N is the size of the `Nlm4Lock` structure (since reading primitives is O(1)).
- Space: O(N), where N is the size of the `Nlm4Lock` structure (due to internal allocations for strings/vectors within the lock).

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `Nlm4CancelArgs` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::primitive`**:
    - `u64`: Reads a Big-Endian unsigned 64-bit integer.
    - `bool`: Reads a 32-bit integer and maps 0 to `false` and 1 to `true`, returning an error otherwise.
- **From `nfs_mamont::nlm::cookie`**:
    - `Cookie::new`: Constructor that wraps a raw `u64` into the `Cookie` type.
- **From `nfs_mamont::parser::nlm` (Assumption based on facts)**:
    - `parse_lock`: A function that deserializes the `Nlm4Lock` structure, which includes variable-length fields like strings and opaque handles.

---

## 4. Data Model

Entities:
- None defined in this module (only a parsing function).

Relations:
- N/A.

Global Invariants:
- The order of fields in the byte stream must strictly follow the XDR definition of `Nlm4CancelArgs`: `cookie` (u64), `block` (bool), `exclusive` (bool), `lock` (Nlm4Lock).

---

## 5. Error Model

Error Types:
- **`Error`**: Re-exported from `crate::parser` (originating from `nfs_mamont::rpc`).

Error Propagation Strategy:
- Custom enum (`Error`). The function uses the `?` operator to propagate errors returned by `u64`, `bool`, and `parse_lock`.

Recoverability:
- Unrecoverable for the current parsing operation. If an error occurs, the stream cursor position is undefined relative to the message boundary, and the operation cannot be retried on the same stream context without resetting.

Panics:
- Allowed: No.
- Conditions: The code relies on `read_exact` and `map_err` for I/O and validation, ensuring no panics occur on invalid input.

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

This module is used in order to deserialize the arguments of the NLMv4 `CANCEL` procedure from a raw network byte stream into a structured Rust representation. In the Network Lock Manager protocol, the `CANCEL` procedure allows a client to withdraw a pending blocked lock request. The server receives this request as a sequence of bytes encoded in XDR format.

This system contains the specific logic required to interpret this byte sequence. It composes low-level primitive parsers (for integers and booleans) with higher-level parsers (for lock details) to reconstruct the `Nlm4CancelArgs` structure. This structure is essential because it contains the `cookie` (to identify the transaction), the `block` and `exclusive` flags (to match the original request), and the `lock` details (to identify the specific resource).

A typical usage scenario of the system involves an RPC server receiving a packet identified as an NLM `CANCEL` call. The server extracts the payload and passes it to the `cancel` function defined in this module. The function reads the stream, validates the XDR format, and produces an `Nlm4CancelArgs` instance. This instance is then passed to the service layer (implementing the `Cancel` trait) to perform the actual logic of removing the lock from the wait queue.

Inside the system the following things happen and they use the `cancel` function to translate the wire format into application logic, ensuring that the server correctly interprets the client's intent to cancel a lock operation based on the strict ordering and typing defined by the NLM protocol specification.