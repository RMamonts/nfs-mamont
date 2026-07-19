<!-- SPEC_HASH: 23b6fca6507913c9fe1170d179e0f87f7d1d9e0ef20148ebd47217de05e88b85 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::access
Rust File: src/serializer/server/nfs/access.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io` and `std::io::Write`**: Used to define the destination sink for the XDR bytes. The `Write` trait allows the serializer to write into any buffer (e.g., network socket, memory buffer) that implements the standard write interface.
- **`crate::serializer::files::file_attr`**: Used to serialize the `file::Attr` structure into the XDR `fattr3` format. This is necessary because the `ACCESS` response includes optional file attributes (`post_op_attr`) which, if present, must be encoded as a full attribute structure.
- **`crate::serializer::{option, u32}`**: Used to handle the specific XDR encoding rules for the fields in the `ACCESS` response. `option` is used to serialize the `post_op_attr` (which is an XDR union: either attributes or a void boolean). `u32` is used to serialize the `access` bitmask, which represents the permissions granted to the user.
- **`crate::vfs::access`**: Used to import the domain-specific result types `access::Success` and `access::Fail`. These structs contain the high-level data (attributes and access mask) produced by the VFS layer that need to be converted to the wire format.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To convert the high-level result of the VFS `ACCESS` procedure check into the binary NFSv3 wire format (`ACCESS3resok` and `ACCESS3resfail`).
- To handle the conditional serialization of file attributes (`post_op_attr`) and the encoding of the access rights bitmask.

Inputs:
- `dest`: A mutable reference to a type implementing `std::io::Write`, representing the destination buffer for the XDR output.
- `arg`: Either `access::Success` (containing the granted access mask and optional attributes) or `access::Fail` (containing optional attributes).

Outputs:
- `io::Result<()>`: Indicates success or failure of the write operation.

Steps:
1. **`result_ok` Serialization**:
 - The function first serializes the `object_attr` field using the `option` helper. This writes a boolean discriminant (true if attributes are present) followed by the `file_attr` data if applicable.
 - The function then serializes the `access` field. It extracts the raw `u32` bits from the `access::Mask` using `arg.access.bits()` and writes them to the destination using the `u32` helper.
2. **`result_fail` Serialization**:
 - The function serializes the `object_attr` field using the `option` helper, identical to the success case. In the NFSv3 protocol, the failure response for `ACCESS` only contains the `post_op_attr` and no access mask.

Edge Cases:
- **Missing Attributes**: If `arg.object_attr` is `None`, the `option` serializer writes a `false` boolean (or equivalent XDR representation for the absent union arm) and skips the attribute serialization.
- **I/O Failure**: If the underlying `Write` implementation returns an error (e.g., buffer full, network failure), the error is propagated immediately via the `?` operator, halting serialization.

Complexity:
- Time: O(1). The size of the serialized data is fixed (a boolean + optional fixed-size attributes + a u32).
- Space: O(1) auxiliary space.

Determinism:
- Deterministic. Given the same input struct, the sequence of bytes written is identical.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer::files`**:
 - **`file_attr`**: This mechanism is responsible for the heavy lifting of converting the internal `file::Attr` representation (mode, size, timestamps, etc.) into the XDR `fattr3` structure. The current module delegates the serialization of the attribute payload to this function whenever attributes are present in the `ACCESS` result.
- **From `nfs_mamont::serializer`**:
 - **`option`**: This mechanism handles the XDR "optional" or "union" logic. For the `ACCESS` response, the attributes are wrapped in a `post_op_attr`, which is technically a union in XDR (either `attributes` or `void`). The `option` function correctly writes the discriminant and conditionally invokes the serialization closure.
 - **`u32`**: This mechanism ensures the access mask is written in Big Endian byte order, as required by the XDR standard.
- **From `nfs_mamont::vfs::access`**:
 - **`Success` and `Fail`**: These structs define the data contract. The `Success` struct specifically contains the `Mask` object, which encapsulates the specific access rights (READ, LOOKUP, MODIFY, etc.) granted by the server. The current module relies on the `bits()` method of this `Mask` to get the raw integer value for serialization.

---

## 4. Data Model

Entities:
- **`result_ok` Function**: Maps `vfs::access::Success` to XDR `ACCESS3resok`.
- **`result_fail` Function**: Maps `vfs::access::Fail` to XDR `ACCESS3resfail`.

Relations:
- **`result_ok` → `file_attr`**: Calls `file_attr` if `Success.object_attr` is `Some`.
- **`result_fail` → `file_attr`**: Calls `file_attr` if `Fail.object_attr` is `Some`.
- **`result_ok` → `u32`**: Calls `u32` to serialize the `Success.access` bitmask.

Global Invariants:
- The output byte stream must conform to RFC 1813 Section 3.3.5 ("ACCESS Procedure").
- The `access` field in `result_ok` must be serialized as a 32-bit unsigned integer representing the bitmask of supported access types.

## 5. Error Model

Error Types:
- **`std::io::Error`**: The only error type produced, originating from the underlying `Write` trait implementation or the helper serializers.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used after calls to `option` and `u32`. Any I/O error encountered during writing is immediately returned to the caller.

Recoverability:
- **Recoverable**: The caller (typically the RPC response handler) can catch the `io::Result`, log the error, and likely terminate the connection or send a generic RPC failure (e.g., `PROC_UNAVAIL` or `SYSTEM_ERR` if the response buffer cannot be written).

Panics:
- **Allowed**: No. The code performs no explicit panicking operations.

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

This module is used in order to **serialize the result of the NFSv3 `ACCESS` procedure into the XDR wire format**. The system contains an NFSv3 server that must communicate with clients using the strict binary protocol defined in RFC 1813. When a client requests an access check (e.g., "Can I read this file?"), the server's Virtual File System (VFS) layer performs the check and returns a high-level Rust result (`access::Success` or `access::Fail`). This result cannot be sent directly to the client because it uses Rust-specific memory layouts and types.

A typical usage scenario of the system involves the server receiving an `ACCESS` request, processing it via the VFS, and obtaining a `Success` struct containing a `Mask` (e.g., `READ | MODIFY`) and optional file attributes. The system then invokes `result_ok` from this module. This function translates the Rust struct into the specific byte sequence defined by the `ACCESS3resok` XDR structure: it writes the optional attributes (using `file_attr`) followed by the 32-bit access mask. Without this module, the server would lack the logic to format this specific response, making it impossible to inform the client about their permissions or update the client's attribute cache.

Inside the system, the following things happen and they use this module:
1. **Response Formatting**: The RPC layer, after dispatching the request to the VFS, receives a `Result<Success, Fail>`. It matches on this result and calls either `result_ok` or `result_fail` to populate the outgoing RPC message buffer.
2. **Protocol Compliance**: The module ensures that the `post_op_attr` (optional attributes) is encoded correctly as an XDR union. This is critical because the NFS protocol allows the server to omit attributes if they are too expensive to retrieve, but the client must be able to distinguish between "attributes present" and "attributes absent" via the boolean discriminant handled by the `option` serializer.
3. **Bitmask Transmission**: The module ensures that the `Mask` (a Rust enum or struct wrapping a `u32`) is flattened into a raw integer on the wire, allowing the client to interpret the specific access bits (READ, LOOKUP, etc.) defined by the protocol.