<!-- SPEC_HASH: 8401d91728a99796b20f813e23a5a0b08f0b391a9d2e3c25582c1398a62837c7 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::set_attr
Rust File: src/serializer/server/nfs/set_attr.rs

---

## 1. Dependencies

- **`std::io::Write`**
  - **Purpose:** Provides the generic output interface for the XDR byte stream. It allows the serializer to write to any target (e.g., network buffers, file streams) that implements the standard `Write` trait.

- **`crate::serializer::files`**
  - **Purpose:** Supplies the `wcc_data` function. This module is used to delegate the actual encoding logic for Weak Cache Consistency (WCC) data, which constitutes the entire payload of both the success and failure response bodies in the NFSv3 `SETATTR` procedure.

- **`crate::vfs::set_attr`**
  - **Purpose:** Defines the domain-specific result types `set_attr::Success` and `set_attr::Fail`. These structs represent the outcome of the VFS `SETATTR` operation and hold the `WccData` that needs to be serialized.

---

## 2. Mechanics

### Serialization of SETATTR3resok (`result_ok`)

**Intent:**
To serialize the successful result body of the NFSv3 `SETATTR` procedure into XDR format.

**Inputs:**
- `dest`: A mutable reference to a type implementing `Write`, serving as the byte sink.
- `arg`: An instance of `set_attr::Success`, containing the `wcc_data` resulting from the operation.

**Outputs:**
- `io::Result<()>`: Indicates successful completion of the write operation or propagates an I/O error.

**Steps:**
1. Extract the `wcc_data` field from the `set_attr::Success` argument.
2. Invoke the `wcc_data` function from `crate::serializer::files`, passing `dest` and the extracted data.
3. Return the result.

**Edge Cases:**
- Propagates any I/O errors encountered during the write operation to the underlying `dest`.

**Complexity:**
- **Time:** O(N), where N is the size of the serialized `WccData` (delegated to `wcc_data`).
- **Space:** O(1) auxiliary space (excluding the output buffer).

**Determinism:**
- Deterministic.

### Serialization of SETATTR3resfail (`result_fail`)

**Intent:**
To serialize the failure result body of the NFSv3 `SETATTR` procedure into XDR format.

**Inputs:**
- `dest`: A mutable reference to a type implementing `Write`.
- `arg`: An instance of `set_attr::Fail`, containing an `error` code and `wcc_data`.

**Outputs:**
- `io::Result<()>`: Indicates success or propagates an I/O error.

**Steps:**
1. Extract the `wcc_data` field from the `set_attr::Fail` argument.
2. Invoke the `wcc_data` function from `crate::serializer::files`, passing `dest` and the extracted data.
3. Return the result.

**Edge Cases:**
- The `error` field within `arg` is explicitly ignored. This implies that the NFS status code (the union discriminator in the XDR response) is serialized by the caller before this function is invoked, or that the `Fail` struct's error field is intended for internal logic/logging rather than the wire protocol body.

**Complexity:**
- **Time:** O(N), where N is the size of the serialized `WccData`.
- **Space:** O(1) auxiliary space.

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

- **From `nfs_mamont::serializer::files`:**
  - **`wcc_data`**: This is the primary mechanism used by this module. It serializes the `vfs::WccData` structure, which contains pre-operation (`before`) and post-operation (`after`) file attributes. This mechanism is crucial for maintaining cache consistency between the client and the server without requiring full re-reads of file attributes.

- **From `nfs_mamont::vfs::set_attr`:**
  - **`set_attr::Success`**: A structure holding the `wcc_data` for a successful operation.
  - **`set_attr::Fail`**: A structure holding an `error` (vfs::Error) and `wcc_data` for a failed operation. The serializer utilizes the `wcc_data` field from this struct.

---

## 4. Data Model

**Entities:**
- **`set_attr::Success`**: Represents the successful outcome of a `SETATTR` VFS operation.
  - Fields: `wcc_data: vfs::WccData`.
- **`set_attr::Fail`**: Represents the failed outcome of a `SETATTR` VFS operation.
  - Fields: `error: vfs::Error`, `wcc_data: vfs::WccData`.

**Relations:**
- `set_attr::Success` → `vfs::WccData` (Composition)
- `set_attr::Fail` → `vfs::WccData` (Composition)

**Global Invariants:**
- None specific to this module.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Represents errors during the writing process (e.g., buffer full, broken pipe).

**Error Propagation Strategy:**
- Direct propagation. Errors returned by the underlying `Write` implementation or the `wcc_data` function are passed up to the caller via the `io::Result` type.

**Recoverability:**
- Not recoverable within this module. The caller must handle I/O failures (e.g., by aborting the connection or logging the error).

**Panics:**
- **Allowed:** No.
- **Conditions:** This module does not introduce any panics. Panics would only originate from the underlying `Write` implementation or the `wcc_data` dependency.

---

## 6. Traits

This module does not implement any external traits. It acts as a consumer of the `std::io::Write` trait.

---

## 7. Overview

This module is used in order to **convert the internal Virtual File System (VFS) results of the NFSv3 `SETATTR` procedure into the standardized XDR wire format**.

The system contains **a layered architecture where the VFS layer handles file system logic and produces Rust structs, while the serializer layer handles protocol encoding**. The `SETATTR` procedure is unique in NFSv3 because both its success (`SETATTR3resok`) and failure (`SETATTR3resfail`) response bodies consist exclusively of Weak Cache Consistency (`wcc_data`) information. The `wcc_data` allows the client to validate and update its attribute cache efficiently.

A typical usage scenario of the system involves:
1. The VFS layer executes a `set_attr` operation and returns a `Result<Success, Fail>`.
2. The RPC layer determines the NFS status code (success or failure).
3. The RPC layer calls either `result_ok` or `result_fail` from this module to serialize the response body.
4. These functions delegate the heavy lifting to `wcc_data`, which writes the pre- and post-operation attributes to the network stream.

Inside the system the following things happen and they use **the `wcc_data` serializer to ensure that the client receives the necessary attribute information to maintain cache consistency, regardless of whether the attribute modification succeeded or failed**. Notably, the `error` field in `set_attr::Fail` is not serialized by `result_fail`; it is assumed that the specific NFS status code corresponding to that error has already been written as the union discriminator by the calling RPC layer. This design separates the status (union tag) from the body (union payload).