<!-- SPEC_HASH: 4c35ba5bb409040e41296c4f8f708e14c1373f08670f400ff4f659d9323129af -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::write
Rust File: src/serializer/server/nfs/write.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io` and `std::io::Write`**: Used to define the interface for the destination byte stream (`dest`). The `Write` trait allows the serializer to write bytes to any buffer (e.g., network socket, memory buffer) in a generic way.
- **`crate::serializer::files::wcc_data`**: Used to serialize the Weak Cache Consistency (WCC) data contained in both the `Success` and `Fail` structs. This dependency handles the complex logic of serializing optional pre- and post-operation attributes.
- **`crate::serializer::{array, u32, variant}`**: Used as low-level building blocks for XDR encoding.
    - `u32`: Serializes the `count` field (number of bytes written).
    - `variant`: Serializes the `StableHow` enum discriminant.
    - `array`: Serializes the `verifier` byte array.
- **`crate::vfs::write`**: Used as the source of the data structures being serialized (`Success`, `Fail`, `StableHow`). These structures represent the outcome of the VFS write operation and need to be converted to the wire format.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To implement the XDR serialization logic specifically for the NFSv3 `WRITE` procedure response.
- To convert the internal VFS result types (`write::Success` and `write::Fail`) into the binary format defined by RFC 1813 (`WRITE3resok` and `WRITE3resfail`).

Inputs:
- `dest`: A mutable reference to a type implementing `std::io::Write`, representing the destination buffer for the XDR bytes.
- `arg`: Either a `write::Success` struct (for successful writes) or a `write::Fail` struct (for failed writes).

Outputs:
- `io::Result<()>`: Indicates successful serialization of the entire structure into the destination or an I/O error if the write operation fails.

Steps:
1. **`stable_how` Serialization**:
   - The function takes a `write::StableHow` enum.
   - It delegates to the generic `variant` function from the parent serializer module, which converts the enum to its `u32` discriminant and writes it to `dest`.

2. **`result_ok` Serialization (Success Path)**:
   - The function accepts a `write::Success` struct.
   - It serializes the `file_wcc` field by calling `wcc_data`.
   - It serializes the `count` field (bytes written) by calling `u32`.
   - It serializes the `committed` field (actual stability level) by calling `stable_how`.
   - It serializes the `verifier` field. Since `verifier` is a newtype struct wrapping a fixed-size array `[u8; NFS3_WRITEVERFSIZE]`, the code accesses the inner array via `.0` and passes it to the `array` serializer.

3. **`result_fail` Serialization (Failure Path)**:
   - The function accepts a `write::Fail` struct.
   - It serializes the `wcc_data` field by calling `wcc_data`. Note that the `Fail` struct contains the error code implicitly via the RPC status header (handled elsewhere), but explicitly contains the WCC data here.

Edge Cases:
- **Verifier Access**: The code explicitly accesses `arg.verifier.0` to pass the raw byte array to the `array` serializer. This relies on the `Verifier` struct in `vfs::write` being a newtype wrapper around a fixed-size array.
- **Error Propagation**: If any underlying serialization call (e.g., `wcc_data`, `u32`) returns an `Err`, that error is immediately propagated up via the `?` operator, aborting the serialization of the remaining fields.

Complexity:
- Time: O(1) relative to the logic in this module (fixed number of fields). The overall complexity depends on the underlying `wcc_data` serializer and the `Write` implementation.
- Space: O(1) auxiliary space.

Determinism:
- Deterministic. Given the same input struct and a functioning `Write` implementation, the output byte sequence is identical.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::write`**:
    - **`Success` and `Fail` Structs**: These define the contract for what data must be serialized. `Success` contains `file_wcc`, `count`, `committed`, and `verifier`. `Fail` contains `wcc_data`. The serializer strictly follows the field order and types defined here.
    - **`StableHow` Enum**: This enum defines the stability levels (`Unstable`, `DataSync`, `FileSync`). The serializer relies on this enum implementing `ToPrimitive` (via `num_derive` in the dependency) so that the `variant` serializer can convert it to a `u32`.

- **From `nfs_mamont::serializer::files`**:
    - **`wcc_data` Function**: This is a critical dependency for handling the Weak Cache Consistency data. Both `result_ok` and `result_fail` delegate the serialization of the WCC attributes to this function, ensuring that the complex logic for optional pre/post attributes is reused and consistent across different NFS procedures.

- **From `nfs_mamont::serializer`**:
    - **`variant` Function**: Used to serialize the `StableHow` enum. It abstracts the conversion of an enum variant to its integer discriminant.
    - **`array` Function**: Used to serialize the `verifier` as a fixed-length opaque byte array.
    - **`u32` Function**: Used to serialize the `count` field as a 32-bit big-endian integer.

---

## 4. Data Model

Entities:
- **`result_ok` Function**: Maps `vfs::write::Success` to XDR `WRITE3resok`.
- **`result_fail` Function**: Maps `vfs::write::Fail` to XDR `WRITE3resfail`.
- **`stable_how` Function**: Maps `vfs::write::StableHow` to XDR `stable_how` enum.

Relations:
- **Composition**: `result_ok` composes `wcc_data`, `u32`, `stable_how`, and `array` to construct the full response message.
- **Mapping**:
    - `vfs::write::Success.file_wcc` -> XDR `wcc_data` (in `resok`).
    - `vfs::write::Success.count` -> XDR `count` (unsigned integer).
    - `vfs::write::Success.committed` -> XDR `stable_how` (enum).
    - `vfs::write::Success.verifier` -> XDR `verifier` (opaque array).
    - `vfs::write::Fail.wcc_data` -> XDR `wcc_data` (in `resfail`).

Global Invariants:
- The serialization order must strictly follow the NFSv3 specification for `WRITE3res`.
- The `verifier` must be serialized as a fixed-size array of length `NFS3_WRITEVERFSIZE`.

## 5. Error Model

Error Types:
- **`std::io::Error`**: The only error type produced or handled by this module.

Error Propagation Strategy:
- **Propagation**: The module uses the `?` operator to propagate errors returned by the underlying serialization functions (`wcc_data`, `u32`, `stable_how`, `array`). It does not generate new errors or modify existing ones.

Recoverability:
- **Recoverable**: Since the functions return `io::Result<()>`, the caller (likely the RPC dispatcher) can catch the error and handle it, for example, by closing the connection or logging the failure.

Panics:
- **Allowed**: No.
- **Conditions**: The code does not contain any `panic!`, `unwrap()`, or `expect()` calls. All potential failures (like I/O errors) are handled via `Result`.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **serialize the result of the NFSv3 WRITE procedure into the XDR wire format**. The system implements an NFSv3 server where the storage logic (VFS) is strictly separated from the network encoding logic. When a client requests to write data to a file, the VFS layer processes the request and returns a result indicating success or failure. This result is a high-level Rust struct (`write::Success` or `write::Fail`) containing semantic information like how many bytes were written, the stability of the data, and cache consistency attributes.

The system needs this module to translate these Rust structs into a flat sequence of bytes that conforms to the NFSv3 protocol specification (RFC 1813). Without this translation, the server could not communicate with standard NFS clients. The module ensures that specific protocol details—such as the order of fields (WCC data before count), the encoding of the stability enum, and the exact format of the write verifier cookie—are handled correctly. It acts as the final step in the request pipeline, taking the logical outcome of the file operation and packaging it for transmission over the network.