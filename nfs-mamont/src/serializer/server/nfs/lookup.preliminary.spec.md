<!-- SPEC_HASH: f35e1d5af0fbe38101486576a279fb2b13f9c711cd14691821dd026b7782b7f5 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::lookup
Rust File: src/serializer/server/nfs/lookup.rs

---

## 1. Dependencies

- **`std::io::Write`**: Used as the abstraction for the output destination (byte stream). The module writes XDR encoded data to any type implementing this trait (e.g., TCP stream, buffer).
- **`crate::serializer::files`**: Provides the primitive serializers `file_handle` and `file_attr`. These are used to encode the complex file system specific types (Handle and Attr) into XDR format.
- **`crate::serializer::option`**: Provides the generic XDR mechanism for serializing optional values. This is required because NFSv3 attributes (`file_attr`, `dir_attr`) are optional fields in the protocol.
- **`crate::vfs::lookup`**: Defines the input data structures `Success` and `Fail`. These structures represent the logical outcome of a VFS lookup operation, which this module transforms into the wire format.

---

## 2. Mechanics

### Mechanism 1: Serialization of Successful Lookup (`result_ok`)

**Intent:**
To convert a successful VFS lookup result into the NFSv3 `LOOKUP3resok` XDR structure. This structure contains the handle of the found file, its attributes, and the post-operation attributes of the directory where it was found.

**Inputs:**
- `dest`: A mutable reference to a writer implementing `std::io::Write`.
- `arg`: A `lookup::Success` struct containing:
  - `file`: The file handle of the found object.
  - `file_attr`: Optional attributes of the found object.
  - `dir_attr`: Optional attributes of the directory after the lookup.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the `file` handle using the `file_handle` function.
2. Serialize the `file_attr` using the `option` helper. If present, it invokes `file_attr` to write the attributes; otherwise, it writes an XDR `false`/zero discriminant.
3. Serialize the `dir_attr` using the `option` helper, similar to step 2.

**Edge Cases:**
- If `file_attr` or `dir_attr` are `None`, the `option` serializer ensures the correct XDR representation for absence (a boolean false) is written without invoking the attribute serializer.

**Complexity:**
- Time: O(1) relative to logic flow, though dependent on the size of the underlying handles/attributes being written.
- Space: O(1) (no heap allocation).

**Determinism:**
- Deterministic. The same input struct produces the exact same byte sequence.

---

### Mechanism 2: Serialization of Failed Lookup (`result_fail`)

**Intent:**
To convert a failed VFS lookup result into the NFSv3 `LOOKUP3resfail` XDR structure. This structure primarily carries the post-operation attributes of the directory to allow the client to verify the directory state despite the failure.

**Inputs:**
- `dest`: A mutable reference to a writer implementing `std::io::Write`.
- `arg`: A `lookup::Fail` struct containing:
  - `error`: The specific VFS error code.
  - `dir_attr`: Optional attributes of the directory after the failed operation.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the `dir_attr` using the `option` helper. If present, it invokes `file_attr`; otherwise, it writes the absence discriminant.

**Edge Cases:**
- The `error` field of `arg` is **not** serialized by this function. It is assumed that the NFS status code (corresponding to this error) is serialized by the generic RPC response wrapper or a higher-level serializer before or after calling this function.

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

- **`crate::serializer::files::file_handle`**: Encodes the opaque file identifier bytes. This is critical as the file handle is the primary return value of a successful lookup.
- **`crate::serializer::files::file_attr`**: Encodes the `fattr3` structure (mode, nlink, uid, gid, size, etc.). This is used for both the found file and the directory attributes.
- **`crate::serializer::option`**: Implements the XDR "optional" encoding logic (a boolean discriminant followed by data if true). This is essential for handling the optional nature of attributes in NFSv3.

---

## 4. Data Model

**Entities:**
- **`lookup::Success`**: Represents the result of a successful directory search.
  - `file`: `file::Handle` - Identifier for the found object.
  - `file_attr`: `Option<file::Attr>` - Metadata of the found object.
  - `dir_attr`: `Option<file::Attr>` - Metadata of the parent directory post-operation.
- **`lookup::Fail`**: Represents the result of a failed directory search.
  - `error`: `vfs::Error` - The reason for failure.
  - `dir_attr`: `Option<file::Attr>` - Metadata of the parent directory post-operation (Weak cache consistency data).

**Relations:**
- `Success` contains one `file::Handle`.
- `Success` and `Fail` both contain optional `file::Attr` related to the directory context.

**Global Invariants:**
- The order of serialization in `result_ok` (file, file attributes, directory attributes) must strictly adhere to the NFSv3 `LOOKUP3resok` specification to ensure interoperability with clients.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Returned if the underlying `Write` stream fails (e.g., broken pipe, buffer full).

**Error Propagation Strategy:**
- Propagation: The `?` operator is used to immediately return any `io::Error` encountered during the serialization of sub-components (`file_handle`, `option`, `file_attr`).
- Strategy: Fail-fast. If any part of the structure cannot be written, the entire serialization aborts.

**Recoverability:**
- Not recoverable at this layer. The caller (likely the RPC handler) must handle the I/O error, potentially closing the connection.

**Panics:**
- Allowed: No.
- Conditions: This module assumes the provided `Write` implementation and sub-serializers (`file_handle`, `file_attr`) do not panic on valid input.

---

## 6. Traits

- **`std::io::Write`**: Implemented by the `dest` argument. The module does not implement traits itself but relies on this trait for output.

---

## 7. Overview

This module is used in order to **serialize the response payload for the NFSv3 LOOKUP procedure** into the XDR (External Data Representation) format required for network transmission.

The system contains a **Virtual File System (VFS) layer** that performs logical file system operations (like searching a directory for a name) and returns Rust structs representing the outcome (`Success` or `Fail`). The VFS layer is agnostic to network protocols. To bridge this gap, the serializer layer (this module) translates these high-level Rust structs into a standardized byte stream.

A typical usage scenario involves the NFS server receiving a `LOOKUP` request from a client. The server invokes the VFS to find the file. If the VFS returns a `Success` object containing the file handle and attributes, the server calls `result_ok` to write this data into the response buffer. If the VFS returns a `Fail` object (e.g., file not found), the server calls `result_fail` to write the directory attributes (for cache consistency) and relies on the RPC layer to write the specific error code.

Inside the system, the following things happen and they use **compositional serialization**:
1. The `result_ok` function orchestrates the serialization of the `LOOKUP3resok` structure.
2. It delegates the encoding of specific types to specialized helpers: `file_handle` for the file identifier and `file_attr` for metadata.
3. It uses the `option` helper to handle the optional fields defined in the NFS protocol, ensuring that `None` values are correctly represented as zero/false on the wire.

**Uncertainty:**
The `result_fail` function accepts a `lookup::Fail` struct which contains an `error` field (`vfs::Error`). However, the implementation of `result_fail` explicitly ignores this field and only serializes `dir_attr`. It is assumed that the specific NFS status code corresponding to `vfs::Error` is serialized by the generic RPC response wrapper (handling the `Result` enum discriminant) rather than by this specific payload serializer. This is consistent with XDR RPC practices where the status is often part of the union discriminant outside the specific arm's body.