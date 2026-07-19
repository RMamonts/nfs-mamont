<!-- SPEC_HASH: cbdba81ea97f167d50c88d74a143bf35ac8431f9c1b99e23fdc61d79ac3bed6e -->
# Module Specification

Module: nfs_mamont::serializer::server::nlm
Rust File: src/serializer/server/nlm/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**: Used to provide the `Write` trait, which defines the interface for the destination byte stream (e.g., a network socket or buffer), and `io::Error`/`ErrorKind` for reporting I/O failures or logical validation errors (such as missing data in a union).
- **`crate::nlm::procedures`**: Used to import the result structures (`Nlm4LockRes`, `Nlm4UnlockRes`, `Nlm4CancelRes`, `Nlm4TestRes`) that encapsulate the data returned by NLM procedure handlers. These structures contain the `cookie` and `stat` fields (and optional `holder` info) that need to be serialized.
- **`crate::nlm::Nlm4Stats`**: Used to access the status enumeration values (e.g., `Granted`, `Denied`) which are written as XDR enum discriminants to indicate the outcome of the lock operation.
- **`crate::serializer`**: Used to import low-level XDR serialization primitives (`u32`, `u64`, `variant`, `vector`). This module relies on these primitives to handle the specifics of Big Endian encoding, padding, and data type conversion.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To convert high-level NLM procedure result structures into the XDR (External Data Representation) binary format required for transmission over the network. This ensures that the server's responses comply with the RFC 1813 standard for NLMv4.

Inputs:
- `dest`: A mutable reference to a type implementing the `std::io::Write` trait, acting as the byte sink.
- `res`: A result structure (`Nlm4LockRes`, `Nlm4UnlockRes`, `Nlm4CancelRes`, or `Nlm4TestRes`) containing the outcome of the NLM operation.

Outputs:
- `io::Result<()>`: Indicates successful writing of the serialized bytes to the destination or an error if the write operation fails or input constraints are violated.

Steps:
1. **Cookie Serialization**: All public functions (`lock_res`, `unlock_res`, `cancel_res`, `test_res`) begin by calling the internal `cookie` helper. This helper extracts the raw `u64` value from the `Cookie` struct and writes it to the destination using the `serializer::u64` primitive.
2. **Status Serialization**: The functions then call the internal `stat` helper. This writes the `Nlm4Stats` enum discriminant to the destination using the `serializer::variant` primitive.
3. **Conditional Union Serialization (Test Only)**:
   - The `test_res` function checks if the status is `Nlm4Stats::Denied`.
   - If `Denied`, it attempts to extract the `holder` information from `res.test_stat.holder`.
   - If `holder` is `None` while status is `Denied`, it returns an `io::Error` with `ErrorKind::InvalidInput`.
   - If `holder` exists, it serializes the holder's fields sequentially:
     - `exclusive` as `u32` (boolean cast).
     - `system_identifier` as `u32`.
     - `opaque_handle` as a variable-length XDR vector (length prefix + bytes + padding).
     - `lock_offset` as `u64`.
     - `lock_length` as `u64`.

Edge Cases:
- **Missing Holder**: In `test_res`, if the status is `Denied` but the `holder` field is `None`, the function returns an `InvalidInput` error rather than writing an incomplete packet.
- **Non-Denied Status**: In `test_res`, if the status is not `Denied` (e.g., `Granted`), the holder information is ignored and not written, adhering to the XDR union definition where the arm is only present for the specific discriminant.

Complexity:
- Time: O(N), where N is the size of the data being written. For fixed-size responses (Lock, Unlock, Cancel), this is O(1). For Test responses with a holder, it depends on the size of the `opaque_handle`.
- Space: O(1) auxiliary space (excluding the buffer managed by the `Write` implementation).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer`**:
 - **`u64`**: Used to write the 64-bit cookie value.
 - **`variant`**: Used to write the 32-bit enum discriminant for `Nlm4Stats`.
 - **`u32`**: Used to write the `exclusive` flag and `system_identifier` of the lock holder.
 - **`vector`**: Used to write the variable-length `opaque_handle` byte array, including the length prefix and necessary padding.
 - **Big Endian Encoding**: The module implicitly relies on the `serializer` module to handle the conversion of Rust integers to network byte order (Big Endian) as required by XDR.

- **From `nfs_mamont::nlm::procedures`**:
 - **`Nlm4TestRes`**: The module accesses the `test_stat` field to determine the status and retrieve the optional `holder` data. The structure of `Nlm4TestRes` (specifically the `Nlm4TestReply` union inside it) dictates the conditional logic in the `test_res` function.

- **From `nfs_mamont::nlm::cookie`**:
 - **`Cookie::raw`**: The module calls this method to retrieve the underlying `u64` value from the `Cookie` wrapper for serialization.

---

## 4. Data Model

Entities:
- This module defines no new public data entities. It operates on entities defined in `crate::nlm::procedures` and `crate::nlm`.

Relations:
- **Serialization Mapping**:
 - `Nlm4LockRes` maps to XDR struct `nlm4stat` + `cookie`.
 - `Nlm4UnlockRes` maps to XDR struct `nlm4stat` + `cookie`.
 - `Nlm4CancelRes` maps to XDR struct `nlm4stat` + `cookie`.
 - `Nlm4TestRes` maps to XDR union `nlm4testr` (cookie + stat + optional holder).

Global Invariants:
- **Union Consistency**: When serializing `Nlm4TestRes`, if the status code indicates `Denied`, the `holder` field within the response structure must be populated (`Some`). If it is `None`, the serializer treats this as a protocol violation and returns an error.

## 5. Error Model

Error Types:
- `std::io::Error`

Error Propagation Strategy:
- **I/O Errors**: Errors returned by the underlying `Write` implementation (e.g., broken pipe) are propagated immediately using the `?` operator.
- **Validation Errors**: The module explicitly constructs `io::Error` with `ErrorKind::InvalidInput` when logical constraints are violated (e.g., `test_res` encountering a `Denied` status without a corresponding `holder`).

Recoverability:
- Recoverable. The functions return `Result`, allowing the RPC layer to catch the error and potentially abort the specific request or close the connection gracefully.

Panics:
- Allowed: No.
- Conditions: The code avoids panics by checking for `None` in optional fields and returning errors instead of unwrapping.

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

This module is used in order to translate the internal result objects of the Network Lock Manager (NLM) service into the standardized XDR byte stream required for communication with NFS clients. The NLM protocol defines specific binary layouts for procedure responses (Lock, Unlock, Cancel, Test). While the business logic of the server operates on high-level Rust structs (like `Nlm4LockRes`), the network layer requires a raw sequence of bytes conforming to RFC 1813.

The system contains a layered architecture where the `nlm` service module handles the logic of granting or denying locks, and the `serializer` module handles the mechanics of encoding data. This module acts as the adapter between these two layers for NLMv4 responses. It ensures that the specific fields of the NLM response structures—such as the transaction cookie, the status code, and the optional lock holder details—are written in the correct order and format.

A typical usage scenario of the system involves a client requesting a lock. The NLM service determines the lock cannot be granted and returns a `Nlm4LockRes` with `stat: Denied`. The RPC layer then invokes `lock_res` from this module, passing the result struct and a network buffer. The module writes the cookie (for correlation) and the `Denied` status code to the buffer. In the case of a `TEST` procedure, if the lock is denied, the module additionally serializes the `Nlm4Holder` details (who holds the lock, the range, etc.) so the client can diagnose the conflict.

Inside the system, the following things happen and they use this module: The `test_res` function specifically implements the logic for an XDR discriminated union. It inspects the `stat` field to decide whether to serialize the subsequent "holder" fields. This conditional logic is critical because writing the holder data when the status is `Granted` would violate the protocol and cause client parsing errors. By centralizing this logic here, the system ensures that the complex wire-format requirements of NLMv4 are met consistently, regardless of how the lock manager internals evolve.