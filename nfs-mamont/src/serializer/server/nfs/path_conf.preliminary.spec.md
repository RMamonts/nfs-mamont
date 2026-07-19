<!-- SPEC_HASH: 65ff71035a9e87230ae718d00c0448314fed6e2c223ba239f813e51d7729454a -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::path_conf
Rust File: src/serializer/server/nfs/path_conf.rs

---

## 1. Dependencies

- **`std::io::Write`**: Core trait used as the destination abstraction for the byte stream. It allows the serializers to write to buffers, sockets, or files without knowing the concrete type.
- **`crate::serializer::files::file_attr`**: Used to serialize the `file_attr` field (metadata) which is present in both success and failure responses. This dependency handles the complex nested structure of file attributes.
- **`crate::serializer::{bool, option, u32}`**: Primitive XDR serializers used to encode basic data types. `option` handles the XDR specific encoding of optional values (a boolean discriminator followed by the value or nothing), while `u32` and `bool` handle the fundamental integer and boolean encoding.
- **`crate::vfs::path_conf`**: Provides the data structures `path_conf::Success` and `path_conf::Fail` which act as the source of truth for the data to be serialized. These structures represent the logical outcome of the VFS `PATHCONF` operation.

---

## 2. Mechanics

### Mechanism 1: Serialization of PATHCONF3resok (`result_ok`)

**Intent:**
To convert a successful VFS `PATHCONF` operation result into the XDR format defined by RFC 1813 (PATHCONF3resok). This involves writing filesystem limits and behavioral flags to the output stream.

**Inputs:**
- `dest`: A mutable reference to a type implementing `std::io::Write`.
- `arg`: A `path_conf::Success` struct containing the filesystem attributes and limits.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the `file_attr` field using the `option` helper. If present, it delegates to `file_attr` serializer.
2. Serialize the `link_max` field (maximum number of hard links) as a `u32`.
3. Serialize the `name_max` field (maximum filename length) as a `u32`.
4. Serialize the `no_trunc` field (whether names are truncated or rejected) as a `bool`.
5. Serialize the `chown_restricted` field (ownership change restrictions) as a `bool`.
6. Serialize the `case_insensitive` field (filename case sensitivity) as a `bool`.
7. Serialize the `case_preserving` field (filename case preservation) as a `bool`.

**Edge Cases:**
- If the `Write` implementation returns an error (e.g., buffer full, broken pipe), the function returns immediately with that error, leaving the stream in a potentially partial state.

**Complexity:**
- Time: O(1) — The number of fields is fixed and small.
- Space: O(1) — No heap allocation occurs within this function.

**Determinism:**
- Deterministic — Given the same input struct and a functioning `Write` implementation, the output byte sequence is identical.

### Mechanism 2: Serialization of PATHCONF3resfail (`result_fail`)

**Intent:**
To convert a failed VFS `PATHCONF` operation result into the XDR format defined by RFC 1813 (PATHCONF3resfail). This primarily involves writing the weak cache consistency data (pre-operation attributes).

**Inputs:**
- `dest`: A mutable reference to a type implementing `std::io::Write`.
- `arg`: A `path_conf::Fail` struct containing the error context and optional attributes.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the `file_attr` field using the `option` helper. If present, it delegates to `file_attr` serializer.
2. Note: The `error` field contained in `path_conf::Fail` is *not* serialized here. It is assumed to be handled by a higher-level procedure status serializer (e.g., the NFS status code wrapper).

**Edge Cases:**
- Same as `result_ok`; I/O errors propagate immediately.

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

- **`nfs_mamont::serializer::option`**: This mechanism is critical for handling fields that may or may not exist. It writes a boolean discriminator (`true` if value exists, `false` otherwise) followed by the value serialization if the discriminator is true.
- **`nfs_mamont::serializer::files::file_attr`**: This mechanism encapsulates the serialization of file metadata (size, mode, times, etc.). It is reused here to handle the `obj_attributes` part of the PATHCONF response.
- **`nfs_mamont::serializer::u32` / `bool`**: These mechanisms handle the Big-Endian encoding required by XDR for 32-bit integers and booleans.

---

## 4. Data Model

**Entities:**
- **`path_conf::Success`**: A structure representing the successful retrieval of path configuration information. It aggregates integer limits (`link_max`, `name_max`) and boolean flags describing filesystem behavior (`no_trunc`, `chown_restricted`, etc.).
- **`path_conf::Fail`**: A structure representing a failed operation. It contains optional pre-operation attributes (`file_attr`) used for cache consistency.

**Relations:**
- `path_conf::Success` contains `Option<file::Attr>`.
- `path_conf::Fail` contains `Option<file::Attr>`.

**Global Invariants:**
- The order of fields in the serialized output must strictly adhere to the NFSv3 XDR specification order to ensure interoperability with clients.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Represents any I/O error encountered during writing to the destination buffer/stream.

**Error Propagation Strategy:**
- Propagation via the `?` operator. Errors are not handled or modified within this module; they are passed directly to the caller.

**Recoverability:**
- Not recoverable within this module. If an I/O error occurs, the serialization process aborts.

**Panics:**
- Allowed: No.
- Conditions: This module assumes that the provided `Write` implementation and the dependency serializers (`file_attr`, `u32`, `bool`) do not panic.

---

## 6. Traits

- **`std::io::Write`**: Used as a trait bound (`impl Write`) for the destination argument.

---

## 7. Overview

This module is used in order to **transform high-level Virtual File System (VFS) results for the `PATHCONF` procedure into the standardized XDR byte stream required by the NFSv3 network protocol**.

The system contains a layered architecture where the **VFS layer** handles the logic of querying filesystem capabilities (like maximum filename length or case sensitivity) and returns Rust structs (`Success` or `Fail`). The **Serializer layer** (this module) acts as the adapter between these internal Rust representations and the external wire format. It ensures that the data is laid out in memory exactly as defined by the NFS RFC 1813 specification, which uses XDR (External Data Representation).

A typical usage scenario of the system involves an NFS server receiving a `PATHCONF` request. The server logic queries the VFS, which returns a `Result<Success, Fail>`. This module is then invoked to write the result into the TCP send buffer. The `result_ok` function is used if the VFS query succeeded, writing filesystem limits and flags. The `result_fail` function is used if the query failed, writing only the weak cache consistency data (attributes), while the specific error code is handled by the surrounding RPC layer.

Inside the system the following things happen and they use **compositional serialization**: The module does not write raw bytes directly for complex structures but delegates to specialized helpers. It uses `option` to handle optional attributes, `file_attr` to handle file metadata, and primitive serializers for integers and booleans. This approach ensures that changes to the XDR definition of sub-structures (like file attributes) automatically propagate to this procedure.