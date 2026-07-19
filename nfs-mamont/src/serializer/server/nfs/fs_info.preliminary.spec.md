<!-- SPEC_HASH: 8da37f59353ec8f0888685996437aa01964415ea36e71aa5c64e07a71ce75fed -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::fs_info
Rust File: src/serializer/server/nfs/fs_info.rs

---

## 1. Dependencies

- **`std::io` & `std::io::Write`**: Provides the `Write` trait, which is the abstraction used for the output destination (e.g., a TCP stream or a buffer). It allows the module to write serialized bytes generically.
- **`crate::vfs::fs_info`**: Defines the domain-specific data structures `Success` and `Fail`. These structures contain the logical data resulting from a file system info operation (e.g., maximum read size, file system properties) that needs to be transmitted.
- **`crate::serializer`**: Supplies primitive XDR serialization utilities (`u32`, `u64`, `option`). These are used to encode basic data types and handle the protocol-specific encoding of optional fields.
- **`crate::serializer::files`**: Provides specialized serializers for file-related types (`file_attr`, `nfs_time`). These are used to encode complex nested structures like file attributes and timestamps within the FSINFO response.

---

## 2. Mechanics

### Mechanism: Serialization of FSINFO3resok (`result_ok`)

**Intent:**
To convert a successful file system information result (`fs_info::Success`) into the XDR (External Data Representation) format defined by the NFSv3 protocol and write it to an output stream.

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait (the byte sink).
- `arg`: A `fs_info::Success` struct containing the file system attributes and limits.

**Outputs:**
- `io::Result<()>`: Indicates success or returns an I/O error if writing to `dest` fails.

**Steps:**
1. Serialize the `root_attr` field using the `option` helper. If present, it invokes `file_attr` to write the attributes; otherwise, it writes the XDR representation for "none".
2. Serialize the `read_max` field as a 32-bit unsigned integer (`u32`).
3. Serialize the `read_pref` field as a 32-bit unsigned integer (`u32`).
4. Serialize the `read_mult` field as a 32-bit unsigned integer (`u32`).
5. Serialize the `write_max` field as a 32-bit unsigned integer (`u32`).
6. Serialize the `write_pref` field as a 32-bit unsigned integer (`u32`).
7. Serialize the `write_mult` field as a 32-bit unsigned integer (`u32`).
8. Serialize the `read_dir_pref` field as a 32-bit unsigned integer (`u32`).
9. Serialize the `max_file_size` field as a 64-bit unsigned integer (`u64`).
10. Serialize the `time_delta` field using the `nfs_time` helper.
11. Serialize the `properties` field by converting the bit mask to a 32-bit unsigned integer using `bits()` and writing it via `u32`.

**Edge Cases:**
- If `arg.root_attr` is `None`, the `option` serializer ensures the correct XDR "null" representation is written.
- If the underlying `dest` stream returns an error during any write operation, the function terminates immediately and propagates the `io::Error`.

**Complexity:**
- Time: O(1) (The function performs a fixed sequence of writes).
- Space: O(1) (No additional heap allocation is performed within this function).

**Determinism:**
- Deterministic (Given the same input struct and a functioning `Write` implementation, the output byte sequence is identical).

### Mechanism: Serialization of FSINFO3resfail (`result_fail`)

**Intent:**
To convert a failed file system information result (`fs_info::Fail`) into the XDR format. In NFSv3, failure responses often contain "Weak Cache Consistency" (WCC) data to allow the client to update its cache without a separate lookup.

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait.
- `arg`: A `fs_info::Fail` struct containing the attributes of the root directory (post-operation attributes or pre-operation attributes depending on the VFS implementation details, though here it is just `root_attr`).

**Outputs:**
- `io::Result<()>`: Indicates success or returns an I/O error.

**Steps:**
1. Serialize the `root_attr` field using the `option` helper. If present, it invokes `file_attr` to write the attributes.

**Edge Cases:**
- Same as `result_ok` regarding `None` handling and I/O errors.

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

- **`crate::serializer::option`**: Handles the XDR encoding for optional values. It typically writes a boolean discriminant (0 for none, 1 for some) followed by the value itself if present. This is crucial for `root_attr` which may not be available on failure.
- **`crate::serializer::u32` / `u64`**: Handles the conversion of Rust integers to their big-endian XDR representation (standard for NFS).
- **`crate::serializer::files::file_attr`**: Serializes the `file::Attr` structure, which includes fields like file type, mode, number of links, owner ID, group ID, size, and file system specific IDs.
- **`crate::serializer::files::nfs_time`**: Serializes the `file::Time` structure, which typically consists of seconds and nanoseconds, into the NFSv3 `nfstime3` format.

---

## 4. Data Model

**Entities:**
- **`fs_info::Success`**: A structure representing the successful outcome of an FSINFO request. It contains transfer size limits (`read_max`, `write_max`, etc.), file system limits (`max_file_size`), time granularity (`time_delta`), and capability flags (`properties`).
- **`fs_info::Fail`**: A structure representing the failed outcome. It primarily contains `root_attr` to provide cache consistency data to the client despite the operation failure.

**Relations:**
- `fs_info::Success` contains an optional `file::Attr` (`root_attr`).
- `fs_info::Fail` contains an optional `file::Attr` (`root_attr`).

**Global Invariants:**
- The order of fields in the serialized output strictly follows the NFSv3 RFC 1813 specification for `FSINFO3resok` and `FSINFO3resfail`.
- All integer fields are encoded as 4 or 8 bytes in Big Endian order.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Represents a failure to write to the destination buffer or stream.

**Error Propagation Strategy:**
- Propagation (using the `?` operator). The module does not attempt to handle or recover from I/O errors; it passes them up to the caller.

**Recoverability:**
- Not recoverable within this module. If a write fails, the serialization process is aborted.

**Panics:**
- Allowed: No.
- Conditions: The code assumes that the provided `Write` implementation does not panic on valid inputs. The logic itself contains no explicit panic points (e.g., `unwrap`, `expect`).

---

## 6. Traits

- **`std::io::Write`**: The `dest` argument must implement this trait. It is used to output the serialized bytes.

---

## 7. Overview

This module is used in order to **serialize the results of the NFSv3 FSINFO procedure into the XDR format for network transmission**.

The system contains **a layered architecture where the VFS (Virtual File System) layer handles logical file system operations and produces high-level Rust structs, while the serializer layer handles the protocol-specific encoding**.

A typical usage scenario of the system involves the NFS server receiving an `FSINFO` RPC request. The server invokes the VFS to retrieve file system capabilities (like maximum read/write sizes or time granularity). The VFS returns a `Result<fs_info::Success, fs_info::Fail>`. This module is then invoked to convert that result into a byte stream. If the operation succeeded, `result_ok` writes the capabilities and attributes; if it failed, `result_fail` writes the available attribute data for cache consistency.

Inside the system the following things happen and they use **primitive serializers (`u32`, `option`) and specific file serializers (`file_attr`, `nfs_time`) to construct the byte stream according to the NFSv3 specification**. This ensures that the client receives a standards-compliant response describing the server's file system characteristics.