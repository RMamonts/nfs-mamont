<!-- SPEC_HASH: 91c5ee3439fc4d8b33f05371eb75f1e6cf21809469069dd680ad99a251cd1edf -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::rm_dir
Rust File: src/serializer/server/nfs/rm_dir.rs

---

## 1. Dependencies

- **`std::io::Write`**: Used as the abstraction for the output destination where the XDR encoded bytes are written. This allows the serializer to write to TCP streams, buffers, or any type implementing the `Write` trait.
- **`crate::serializer::files::wcc_data`**: A dependency providing the specific XDR serialization logic for the `WccData` structure. This module delegates the complex encoding of Weak Cache Consistency data to this function.
- **`crate::vfs::rm_dir`**: Provides the domain-specific result types (`Success` and `Fail`) that represent the outcome of a directory removal operation in the Virtual File System layer. These types serve as the input data models for serialization.

---

## 2. Mechanics

### `result_ok`

Intent:
To serialize the successful result body of an NFSv3 `RMDIR` procedure call. This involves encoding the Weak Cache Consistency (WCC) data to allow the client to update its cache information regarding the directory's parent.

Inputs:
- `dest`: A mutable reference to a writer implementing `std::io::Write`.
- `arg`: A `rm_dir::Success` structure containing the `wcc_data` field.

Outputs:
- `io::Result<()>`: Indicates successful completion of the write operation or an I/O error.

Steps:
1. Extract the `wcc_data` field from the `rm_dir::Success` argument.
2. Invoke the `wcc_data` helper function, passing the destination writer and the extracted `wcc_data`.
3. Return the result of the `wcc_data` function.

Edge Cases:
- Relies on the underlying `wcc_data` function to handle the specifics of encoding `Option` types for pre- and post-operation attributes.

Complexity:
- Time: O(1) relative to the logic in this module (delegates entirely to `wcc_data`).
- Space: O(1) stack space.

Determinism:
- Deterministic.

### `result_fail`

Intent:
To serialize the failure result body of an NFSv3 `RMDIR` procedure call. Even on failure, NFSv3 requires returning WCC data for the directory to help the client synchronize its cache state.

Inputs:
- `dest`: A mutable reference to a writer implementing `std::io::Write`.
- `arg`: A `rm_dir::Fail` structure containing the `dir_wcc` field.

Outputs:
- `io::Result<()>`: Indicates successful completion of the write operation or an I/O error.

Steps:
1. Extract the `dir_wcc` field from the `rm_dir::Fail` argument.
2. Invoke the `wcc_data` helper function, passing the destination writer and the extracted `dir_wcc`.
3. Return the result of the `wcc_data` function.

Edge Cases:
- The `error` field present in `rm_dir::Fail` is not serialized by this function. In the NFSv3 protocol, the error status is typically serialized in the union discriminant (header) rather than the failure body structure.

Complexity:
- Time: O(1) relative to the logic in this module (delegates entirely to `wcc_data`).
- Space: O(1) stack space.

Determinism:
- Deterministic.

---

## 3. Dependency Mechanics

- **`nfs_mamont::serializer::files::wcc_data`**:
    - This is the primary mechanism used by this module. It is responsible for the actual XDR encoding of the `vfs::WccData` structure. This structure contains `before` (pre-operation attributes) and `after` (post-operation attributes) fields. The `wcc_data` function handles the logic for serializing these optional fields according to the NFSv3 specification.

- **`nfs_mamont::vfs::rm_dir::Success` / `Fail`**:
    - These structures define the data contract between the VFS layer and the serializer. `Success` holds `wcc_data`, and `Fail` holds `dir_wcc`. The serializer acts as an adapter, transforming these Rust structs into the wire format.

---

## 4. Data Model

Entities:
- **`rm_dir::Success`**: A container for the result of a successful directory removal. It holds `wcc_data` representing the state of the directory's parent.
- **`rm_dir::Fail`**: A container for the result of a failed directory removal. It holds `dir_wcc` (WCC data for the directory) and an `error` code (though the error code is handled outside this specific serializer).

Relations:
- `Success` contains `WccData` (1:1 composition).
- `Fail` contains `WccData` (1:1 composition).

Global Invariants:
- None specific to this module.

---

## 5. Error Model

Error Types:
- **`std::io::Error`**: Represents any error that occurs during the write operation to the destination `dest`.

Error Propagation Strategy:
- The module uses direct propagation. Errors returned by the `wcc_data` function or the underlying `Write` trait are returned immediately to the caller.

Recoverability:
- Not recoverable within this module. The caller must handle the I/O error (e.g., by aborting the connection or logging the failure).

Panics:
- Allowed: No.
- Conditions: This module performs no operations that should panic (e.g., no array indexing, no unwrapping). It assumes the provided `Write` implementation and `wcc_data` function are well-behaved.

---

## 6. Traits

- **`std::io::Write`**: Used by the public functions (`result_ok`, `result_fail`) as a trait bound (`&mut impl Write`) for the destination argument.

---

## 7. Overview

This module is used in order to serialize the response payload for the NFSv3 `RMDIR` (Remove Directory) procedure. It serves as a specific adapter within the larger NFS server serialization layer, converting high-level Virtual File System (VFS) result types into the XDR (External Data Representation) format required for network transmission.

The system contains a separation between the VFS logic (which performs file system operations and returns Rust structs) and the Serializer logic (which converts these structs into bytes). This separation allows the VFS to remain agnostic to network protocols.

A typical usage scenario involves the server receiving an `RMDIR` request, invoking the VFS to remove the directory, and receiving a `Result<Success, Fail>`. Based on this result, the server calls either `result_ok` or `result_fail` from this module. The module then extracts the Weak Cache Consistency (WCC) data—which describes the state of the directory before and after the operation attempt—and serializes it. This WCC data is critical for NFS clients to maintain cache consistency without needing to perform explicit attribute lookup requests immediately after a modification attempt.

Inside the system, the following things happen: The module delegates the complex byte-level encoding of the WCC structure to the `crate::serializer::files::wcc_data` function. This module focuses solely on selecting the correct field (`wcc_data` vs `dir_wcc`) from the VFS result types and passing it to the generic serializer.

**Uncertainty**: The specification for `crate::serializer::files` was not provided, only its public interface. Therefore, it is assumed that `wcc_data` correctly implements the XDR encoding for `vfs::WccData` (handling optional `before` and `after` attributes as defined in RFC 1813). It is also assumed that the `error` field in `rm_dir::Fail` is serialized elsewhere in the RPC response structure (specifically in the `stat` field of the union), as it is not touched by the functions in this module.