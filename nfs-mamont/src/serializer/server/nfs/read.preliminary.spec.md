<!-- SPEC_HASH: 0771d30aec032a00a965bf846360075c0751df74e384214bc087520d98b9bcca -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::read
Rust File: src/serializer/server/nfs/read.rs

---

## 1. Dependencies

- **`std::io::Write`**: Used as the destination trait for the serialized byte stream. It allows the module to write XDR data to any target that implements the `Write` trait (e.g., network sockets, buffers).
- **`crate::serializer` (mod)**: Provides primitive XDR serialization helpers.
  - `option`: Used to serialize optional fields (`Option<T>`) according to XDR discriminated union rules (a boolean presence flag followed by the value if present).
  - `u32`: Used to serialize the `count` field (number of bytes read) as a 32-bit unsigned integer.
  - `bool`: Used to serialize the `eof` (end-of-file) flag.
- **`crate::serializer::files`**: Provides specific serializers for file system related types.
  - `file_attr`: Used to serialize the `file::Attr` structure, which represents file attributes (metadata) in the NFSv3 protocol.
- **`crate::vfs::read`**: Provides the data structures that act as the source for serialization.
  - `SuccessPartial`: Represents the successful result of a read operation excluding the data payload.
  - `Fail`: Represents the failed result of a read operation.

---

## 2. Mechanics

### Mechanism 1: Serialization of Successful Read Result (Partial)

**Intent:**
To serialize the metadata portion of a successful NFSv3 `READ` response (`READ3resok`) into XDR format. This specifically excludes the actual file data payload, which is handled separately.

**Inputs:**
- `dest`: A mutable reference to a type implementing `std::io::Write`.
- `arg`: An instance of `read::SuccessPartial` containing:
  - `file_attr`: `Option<file::Attr>` (Post-operation attributes).
  - `count`: `u32` (Number of bytes returned).
  - `eof`: `bool` (End-of-file flag).

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the `file_attr` field using the `option` helper. This writes a boolean presence flag. If `Some`, it invokes the `file_attr` serializer to write the attributes.
2. Serialize the `count` field using the `u32` helper.
3. Serialize the `eof` field using the `bool` helper.

**Edge Cases:**
- If `file_attr` is `None`, the `option` serializer writes a `0` (false) and skips the attribute serialization.

**Complexity:**
- Time: O(1) (Fixed number of fields written).
- Space: O(1) (No heap allocation within this function).

**Determinism:**
- Deterministic. The same input struct produces the exact same byte sequence.

### Mechanism 2: Serialization of Failed Read Result

**Intent:**
To serialize the metadata portion of a failed NFSv3 `READ` response (`READ3resfail`) into XDR format.

**Inputs:**
- `dest`: A mutable reference to a type implementing `std::io::Write`.
- `arg`: An instance of `read::Fail` containing:
  - `file_attr`: `Option<file::Attr>` (Post-operation attributes, which may be present even on failure).

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the `file_attr` field using the `option` helper.

**Edge Cases:**
- If `file_attr` is `None`, the `option` serializer writes a `0` (false).

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

- **`nfs_mamont::serializer::option`**: Encodes an `Option<T>` as a 4-byte boolean (0 for false, 1 for true) followed by the encoded value of `T` if the boolean is true.
- **`nfs_mamont::serializer::u32`**: Encodes a `u32` integer as 4 bytes in big-endian (network) byte order.
- **`nfs_mamont::serializer::bool`**: Encodes a `bool` as 4 bytes (0 for false, 1 for true), consistent with XDR encoding for enumerations/booleans.
- **`nfs_mamont::serializer::files::file_attr`**: Encodes the `fattr3` structure, which includes file type, mode, number of links, user ID, group ID, size, space used, device ID, file system ID, file ID, and access/modification/change times.

---

## 4. Data Model

**Entities:**
- **`read::SuccessPartial`**: A structure representing the result of a successful read operation excluding the data buffer.
  - `file_attr`: Optional file attributes after the read.
  - `count`: The number of bytes read.
  - `eof`: A flag indicating if the end of file was reached.
- **`read::Fail`**: A structure representing a failed read operation.
  - `file_attr`: Optional file attributes (which might be available depending on the error type).

**Relations:**
- None directly within this module; it acts as a mapper from VFS entities to a byte stream.

**Global Invariants:**
- The serialized output must conform to the NFSv3 XDR specification for `READ3resok` and `READ3resfail`.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Propagated from the underlying `Write` trait implementation or from the sub-serializers (`file_attr`, `u32`, `bool`, `option`).

**Error Propagation Strategy:**
- Fail-fast. Any error encountered during writing immediately returns the `Err` variant to the caller, aborting the serialization process.

**Recoverability:**
- Not recoverable at this layer. The caller must handle the I/O error (e.g., by closing the connection or logging the failure).

**Panics:**
- Allowed: No.
- Conditions: This module assumes that the provided `Write` implementation and sub-serializers do not panic on valid input.

---

## 6. Traits

- **`std::io::Write`**: Used by the public functions (`result_ok_part`, `result_fail`) via the `dest` argument to output the serialized bytes.

---

## 7. Overview

This module is used in order to convert the internal representation of an NFSv3 READ operation result (generated by the VFS layer) into the standardized XDR byte stream required by the NFS protocol. It specifically handles the serialization of the "metadata" portion of the response, such as the number of bytes read, the end-of-file flag, and optional file attributes.

The system contains a layered architecture where the top layer handles network I/O, the middle layer (this module) handles protocol encoding (XDR), and the bottom layer (VFS) handles file system operations. This separation allows the VFS to work with native Rust types while the serializer ensures compliance with the external NFSv3 wire format.

A typical usage scenario of the system involves the VFS layer performing a read operation on a file. It returns a `SuccessPartial` struct containing the count of bytes read and an EOF flag. This module takes that struct and serializes it into a byte sequence. The actual file data payload is handled separately (likely via a zero-copy mechanism or a different serializer), as indicated by the exclusion of the data buffer in `result_ok_part`.

Inside the system the following things happen and they use the `serializer` primitives:
1. The VFS returns a `SuccessPartial` or `Fail` struct.
2. `result_ok_part` or `result_fail` is invoked with a network stream writer.
3. The module uses the `option` serializer to conditionally write file attributes.
4. The module uses `u32` and `bool` serializers to write the count and EOF flags.
5. The resulting bytes are flushed to the network client.