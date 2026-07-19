<!-- SPEC_HASH: cbdba81ea97f167d50c88d74a143bf35ac8431f9c1b99e23fdc61d79ac3bed6e -->
# Module Specification

Module: nfs_mamont::serializer::server::nlm
Rust File: src/serializer/server/nlm/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **Standard Library (`std::io`)**:
    *   `Write`: Used as a trait bound for the destination buffer (`dest`), allowing the serializer to write bytes into any type that implements `Write` (e.g., `TcpStream`, `Vec<u8>`, `Cursor`).
    *   `ErrorKind`: Used to construct an `io::Error` specifically `InvalidInput` when logical inconsistencies are detected in the input data (e.g., a `Denied` status without a lock holder).
*   **`crate::nlm::procedures`**:
    *   `Nlm4LockRes`, `Nlm4UnlockRes`, `Nlm4CancelRes`, `Nlm4TestRes`: These are the source data structures containing the results of NLM operations (cookies, status codes, and holder information) that need to be converted into the wire format.
*   **`crate::nlm::Nlm4Stats`**:
    *   Used to determine the status code to be written on the wire. It is also used in conditional logic (specifically in `test_res`) to decide whether to serialize additional lock holder information.
*   **`crate::nlm::cookie::Cookie`**:
    *   Used to extract the raw `u64` transaction identifier that must be echoed back to the client in the response.
*   **`crate::serializer`**:
    *   `u32`, `u64`: Used to write fixed-width integers in XDR format (big-endian).
    *   `variant`: Used to write the discriminant of the `Nlm4Stats` enum as a 32-bit integer.
    *   `vector`: Used to serialize the opaque byte array (handle) of the lock holder, which includes a length prefix and padding.
*   **Assumption on `crate::nlm::holder::Nlm4Holder`**:
    *   Although the specification for this module was not provided, the code uses it. It is assumed to have the fields: `exclusive` (bool-like), `system_identifier` (integer), `opaque_handle` (byte array), `lock_offset` (u64), and `lock_length` (u64).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in the module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To serialize the result structures of NLM v4 procedures into the XDR (External Data Representation) format required for transmission over the network. This ensures that the server's internal representation of lock states is correctly translated into the standardized byte layout defined by the NFS/NLM protocols.

Inputs:
- `dest: &mut impl Write`: A mutable reference to a byte sink (e.g., a network stream or buffer).
- `res`: A result structure (`Nlm4LockRes`, `Nlm4UnlockRes`, `Nlm4CancelRes`, or `Nlm4TestRes`) containing the operation outcome.

Outputs:
- `io::Result<()>`: Indicates success or an I/O error encountered during writing. Returns `InvalidInput` if the data structure violates protocol invariants (e.g., `Denied` status with no holder info).

Steps:
1.  **Cookie Serialization**: The `cookie` function is called (or inlined logic is used) to extract the raw `u64` from the `Cookie` struct and write it as an XDR `hyper` (unsigned 64-bit integer).
2.  **Status Serialization**: The `stat` function is called to write the `Nlm4Stats` enum as an XDR integer discriminant.
3.  **Conditional Serialization (Test Result)**:
    - If the procedure is `test_res` and the status is `Denied`, the code enters a branch to serialize the conflicting lock holder.
    - It validates that the `holder` field is `Some`; otherwise, it returns an `InvalidInput` error.
    - It serializes the holder's `exclusive` flag, `system_identifier`, `opaque_handle` (as a variable-length byte vector), `lock_offset`, and `lock_length` using the appropriate XDR primitive writers.

Edge Cases:
- **Missing Holder on Denied**: In `test_res`, if the status is `Denied` but the `holder` field is `None`, the function returns an `io::Error` with `ErrorKind::InvalidInput`. This enforces the protocol requirement that a denial must explain who holds the lock.
- **Non-Denied Status with Holder**: If the status is not `Denied` (e.g., `Granted`), the `holder` field (even if present in memory) is ignored and not serialized, adhering to the XDR union definition for the test result.

Complexity:
- Time: O(1) for `lock_res`, `unlock_res`, `cancel_res`. O(N) for `test_res` where N is the size of the `opaque_handle` (typically constant bounded by protocol).
- Space: O(1) auxiliary space (excluding the destination buffer).

Determinism:
- Deterministic. Given the same input structures and destination, the exact byte sequence written is identical.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer`**:
    - `u64(dest, n)`: Writes a 64-bit integer in big-endian format. Used for the cookie and lock ranges.
    - `variant(dest, val)`: Writes the integer representation of an enum discriminant. Used for the `Nlm4Stats` status code.
    - `vector(dest, bytes)`: Writes a byte array prefixed by its length (u32) and padded to a 4-byte boundary. Used for the `opaque_handle`.
- **From `nfs_mamont::nlm::cookie`**:
    - `Cookie::raw()`: Accessor to retrieve the underlying `u64` value for serialization.
- **From `nfs_mamont::nlm`**:
    - `Nlm4Stats`: The enum providing the discriminant values (e.g., `Granted` = 0, `Denied` = 1).

---

## 4. Data Model

Entities:
- None defined in this module. The module operates on entities defined in `crate::nlm::procedures` and `crate::nlm`.

Relations:
- None defined in this module.

Global Invariants:
- **XDR Compliance**: All output must conform to RFC 1014 (XDR) and RFC 1813 (NLM v4). This implies big-endian byte order and 4-byte alignment for certain structures.
- **Protocol Consistency**: For `Nlm4TestRes`, a `stat` value of `Denied` must be accompanied by a valid `holder` structure in the serialized output.

## 5. Error Model

Error Types:
- `std::io::Error`: The primary error type returned by all public functions.

Error Propagation Strategy:
- **Propagation**: Errors from the underlying `Write` trait or the `serializer` helper functions are propagated directly using the `?` operator.
- **Generation**: The module generates `io::Error` with `ErrorKind::InvalidInput` when it detects a logical inconsistency in the input data (specifically in `test_res`).

Recoverability:
- **I/O Errors**: Generally unrecoverable for the specific RPC call being processed; the connection or buffer operation failed.
- **InvalidInput**: Indicates a bug in the server logic (constructing an invalid response structure). The operation cannot be completed successfully as the data violates the protocol contract.

Panics:
- Allowed: No.
- Conditions: The code performs explicit checks (e.g., `match res.test_stat.holder`) and returns `Err` instead of panicking on unexpected `None` values.

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

This module is used in order to translate the high-level, type-safe results of Network Lock Manager (NLM) operations into the raw byte stream format required by the XDR (External Data Representation) standard for network transmission. In an NFS environment, the server processes lock requests using internal Rust structs (like `Nlm4LockRes`), but the client expects a specific sequence of bytes defined by the protocol RFCs. This module serves as the "presentation layer" serializer that bridges this gap.

This system contains a set of specialized serializers for the four main NLM procedures: `LOCK`, `UNLOCK`, `CANCEL`, and `TEST`. It relies on the `serializer` module to handle low-level byte ordering (big-endian) and padding, and on the `nlm` module to provide the data structures (`Cookie`, `Nlm4Stats`, `Nlm4Holder`) that hold the semantic meaning of the response. The `test_res` function is particularly significant as it implements the XDR "union" logic, where the presence of lock holder data on the wire is conditional on the status code being `Denied`.

A typical usage scenario of the system involves the NFS server completing a request to test a lock. The business logic layer produces an `Nlm4TestRes` indicating the lock is denied and provides details of the conflicting owner. The RPC handler then invokes `test_res`, passing a network buffer. This function writes the transaction cookie, the status code, and the conflicting owner's details into the buffer. The buffer is then sent over the network to the client.

Inside the system the following things happen and they use the `Write` trait to abstract the destination, allowing the serialization logic to be unit tested with memory buffers (`Cursor`) while being used in production with TCP streams. The strict validation in `test_res` (returning an error if a holder is missing) ensures that the server does not send malformed responses that could confuse or crash the client.