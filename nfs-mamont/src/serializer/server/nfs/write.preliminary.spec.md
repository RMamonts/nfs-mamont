<!-- SPEC_HASH: 4c35ba5bb409040e41296c4f8f708e14c1373f08670f400ff4f659d9323129af -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::write
Rust File: src/serializer/server/nfs/write.rs

---

## 1. Dependencies

- **`std::io` & `std::io::Write`**: Provides the `Write` trait, which defines the interface for the destination byte stream (e.g., a TCP stream or a buffer) where the XDR data will be written. It also provides the `io::Result` type for error handling.
- **`crate::vfs::write`**: Supplies the domain-specific data structures that represent the outcome of a VFS write operation. Specifically, it provides `write::Success` (containing `file_wcc`, `count`, `committed`, `verifier`), `write::Fail` (containing `wcc_data`), and the `write::StableHow` enum. These are the source types to be serialized.
- **`crate::serializer::files::wcc_data`**: A helper function responsible for serializing Weak Cache Consistency (WCC) data. This is used to serialize the `file_wcc` field in the success case and the `wcc_data` field in the failure case.
- **`crate::serializer::{array, u32, variant}`**: Primitive XDR serialization utilities. `u32` serializes 32-bit unsigned integers, `array` serializes fixed-size byte arrays, and `variant` serializes enum discriminants (integers representing enum variants).

---

## 2. Mechanics

### Mechanism 1: Serialization of Successful Write Response (`result_ok`)

**Intent:**
To convert a successful VFS write operation result (`write::Success`) into the XDR format defined for the `WRITE3resok` structure in the NFSv3 protocol.

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait (the output buffer).
- `arg`: An instance of `write::Success`, containing the result of the write operation.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations to the destination.

**Steps:**
1. Serialize the Weak Cache Consistency data (`arg.file_wcc`) by calling `wcc_data(dest, arg.file_wcc)`.
2. Serialize the number of bytes written (`arg.count`) as a 32-bit unsigned integer by calling `u32(dest, arg.count)`.
3. Serialize the stability indicator (`arg.committed`) by calling `stable_how(dest, arg.committed)`. This internally converts the `StableHow` enum to its XDR discriminant.
4. Serialize the write verifier (`arg.verifier.0`) as a fixed-size byte array by calling `array(dest, arg.verifier.0)`.

**Edge Cases:**
- If any underlying call to `dest.write_all` (inside the helper functions) fails, the function immediately returns the `io::Error`.

**Complexity:**
- Time: O(1), as the size of the data structure is fixed by the NFSv3 protocol.
- Space: O(1), no additional significant allocation is performed within this function (allocations depend on the `Write` implementation).

**Determinism:**
- Deterministic. Given the same input `arg` and a functioning `dest`, the sequence of bytes written is identical.

### Mechanism 2: Serialization of Failed Write Response (`result_fail`)

**Intent:**
To convert a failed VFS write operation result (`write::Fail`) into the XDR format defined for the `WRITE3resfail` structure in the NFSv3 protocol.

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait.
- `arg`: An instance of `write::Fail`, containing the error and WCC data.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the Weak Cache Consistency data (`arg.wcc_data`) by calling `wcc_data(dest, arg.wcc_data)`.

**Edge Cases:**
- Propagates IO errors from the underlying writer.

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

### Mechanism 3: Enum Discriminant Serialization (`stable_how`)

**Intent:**
To serialize the `StableHow` enum into its XDR integer representation.

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait.
- `how`: A `write::StableHow` enum variant.

**Outputs:**
- `io::Result<()>`.

**Steps:**
1. Delegate to the generic `variant` serializer, converting the enum to a primitive integer.

---

## 3. Dependency Mechanics

- **`crate::vfs::write`**:
    - **`write::Success`**: Defines the fields `file_wcc`, `count`, `committed`, and `verifier`. The serializer relies on the exact order and types of these fields to match the NFSv3 specification.
    - **`write::Fail`**: Defines the fields `error` and `wcc_data`. Note that the `error` field is *not* serialized by `result_fail` in this module; it is assumed to be serialized by the caller (likely a generic wrapper handling the status code for all NFS procedures).
    - **`write::StableHow`**: An enum with variants `Unstable`, `DataSync`, and `FileSync`. The `variant` serializer maps these to integer values (likely 0, 1, 2).

- **`crate::serializer`**:
    - **`u32`**: Encodes a `u32` in big-endian format (standard XDR).
    - **`array`**: Encodes a fixed-size slice of bytes. The size is determined at compile time by the type of `verifier.0` (which is `[u8; NFS3_WRITEVERFSIZE]`).
    - **`variant`**: Uses the `ToPrimitive` trait to convert an enum to an integer and then serializes it as a `u32`.

- **`crate::serializer::files`**:
    - **`wcc_data`**: Serializes the `WccData` struct, which contains optional pre-operation attributes and optional post-operation attributes. This is crucial for cache coherency in NFS.

---

## 4. Data Model

**Entities:**
- **`write::Success`**: Represents a successful write. Contains:
    - `file_wcc`: `vfs::WccData` (Attributes before and after the write).
    - `count`: `u32` (Number of bytes written).
    - `committed`: `write::StableHow` (Stability of the data on the server).
    - `verifier`: `write::Verifier` (Cookie to verify server state).
- **`write::Fail`**: Represents a failed write. Contains:
    - `wcc_data`: `vfs::WccData` (Attributes before and after the attempt).
- **`write::StableHow`**: Enum indicating data stability (`Unstable`, `DataSync`, `FileSync`).

**Relations:**
- `result_ok` consumes `write::Success`.
- `result_fail` consumes `write::Fail`.

**Global Invariants:**
- The output byte stream must conform to RFC 1813 (NFS Version 3 Protocol Specification) XDR definition for `WRITE3res`.
- The `verifier` is a fixed-size array (`NFS3_WRITEVERFSIZE`), defined in the VFS module but used here as a byte array.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Represents any error that occurs while writing to the destination buffer (e.g., broken pipe, buffer full).

**Error Propagation Strategy:**
- The `?` operator is used to propagate errors immediately from the helper serialization functions (`wcc_data`, `u32`, `array`, `variant`) to the caller.

**Recoverability:**
- Serialization is generally treated as an atomic step in the request-response cycle. If an IO error occurs here, it typically indicates a failure to send the response to the client, which is usually fatal to that specific RPC request.

**Panics:**
- Allowed: No explicit panics are introduced in this module.
- Conditions: Panics may occur if the underlying `Write` implementation panics or if the `ToPrimitive` conversion in `variant` fails (though `StableHow` is explicitly defined to support this).

---

## 6. Traits

- **`std::io::Write`**: Used by the public functions (`result_ok`, `result_fail`) via the `dest` parameter. The module does not implement this trait but requires it as a bound.

---

## 7. Overview

This module is used in order to **convert the internal result of a file write operation into the standardized XDR wire format required by the NFSv3 protocol**.

This system contains **a set of serializers that translate high-level Virtual File System (VFS) operation results into byte sequences**. The VFS layer handles the logic of file manipulation (permissions, disk writing, attribute updates) and returns Rust structs representing the outcome. The network layer requires raw bytes to send back to the client. This module bridges that gap specifically for the `WRITE` procedure.

A typical usage scenario of the system involves the server processing a `WRITE` request from an NFS client. The server calls the VFS to write data. If the VFS returns a `Result::Ok(write::Success)`, the server calls `result_ok` to serialize the success data (bytes written, stability guarantee, and verifier) into the response buffer. If the VFS returns a `Result::Err(write::Fail)`, the server calls `result_fail` to serialize the failure data (specifically the WCC data to allow the client to update its cache even if the write failed).

Inside the system the following things happen and they use **primitive serializers (`u32`, `array`) and composite serializers (`wcc_data`) to construct the response**. The `result_ok` function ensures that the data is written in the strict order required by the protocol: WCC data, count, stability, and verifier. This ensures the client can correctly interpret the server's response regarding the durability and extent of the written data.