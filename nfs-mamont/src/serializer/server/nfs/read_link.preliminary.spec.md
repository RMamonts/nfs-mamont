<!-- SPEC_HASH: e80b142f3aeafec5a19ba79221d3053833201a291e1c83f932f8ccb4490db99c -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::read_link
Rust File: src/serializer/server/nfs/read_link.rs

---

## 1. Dependencies

- **`std::io` and `std::io::Write`**: Core I/O primitives used to define the output destination for the serialization process. The `Write` trait is the abstraction over byte streams (buffers, sockets, etc.) where the XDR bytes are written.
- **`crate::serializer::files`**: Provides specific serializers for file system related types. Specifically, `file_attr` is used to serialize file attributes, and `file_path` is used to serialize the path data contained within a symbolic link.
- **`crate::serializer`**: Provides generic XDR serialization primitives. The `option` function is used to serialize optional fields (specifically `symlink_attr`) according to XDR rules (encoding a boolean discriminator followed by the value if present).
- **`crate::vfs::read_link`**: Defines the domain-specific data structures (`Success` and `Fail`) that represent the outcome of a VFS `READLINK` operation. This module consumes these structures to produce the wire format.

---

## 2. Mechanics

### Mechanism 1: Serialization of READLINK3resok (`result_ok`)

**Intent:**
To convert a successful VFS `read_link` operation result into the XDR format defined by the NFSv3 specification for `READLINK3resok`. This involves encoding the post-operation attributes of the symlink and the actual path data the symlink points to.

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait, acting as the byte sink.
- `arg`: A `read_link::Success` struct containing:
  - `symlink_attr`: An optional `file::Attr` representing the attributes of the symlink after the operation.
  - `data`: A `file::Path` representing the content of the symbolic link.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the optional `symlink_attr` field using the `option` helper. This helper writes a boolean discriminator (true/false) to indicate presence. If present, it invokes the provided closure which calls `file_attr` to write the attribute structure.
2. Serialize the `data` field using the `file_path` helper, which writes the variable-length byte array or string representing the path.

**Edge Cases:**
- If `symlink_attr` is `None`, the `option` helper writes the XDR representation for `false` (absence) and skips the attribute serialization.
- If the underlying `Write` stream returns an error (e.g., buffer full, broken pipe), the function propagates this error immediately, aborting serialization.

**Complexity:**
- Time: O(N), where N is the size of the attributes (if present) plus the length of the path data.
- Space: O(1) auxiliary space (excluding the output buffer).

**Determinism:**
- Deterministic. The output bytes are strictly determined by the input `arg` structure.

---

### Mechanism 2: Serialization of READLINK3resfail (`result_fail`)

**Intent:**
To convert a failed VFS `read_link` operation result into the XDR format defined by the NFSv3 specification for `READLINK3resfail`. This structure only contains the post-operation attributes of the symlink, as the error status is handled by the enclosing RPC union.

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait.
- `arg`: A `read_link::Fail` struct containing:
  - `symlink_attr`: An optional `file::Attr`.
  - `error`: A `vfs::Error` (Note: This field is present in the struct but is **not** serialized by this function, as per NFSv3 protocol where the status code is separate from the fail body).

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the optional `symlink_attr` field using the `option` helper, identical to the logic in `result_ok`.

**Edge Cases:**
- If `symlink_attr` is `None`, the absence is encoded.
- The `error` field within `arg` is explicitly ignored during serialization. It is assumed that the caller (or a higher-level serializer) handles the encoding of the error status integer.

**Complexity:**
- Time: O(N), where N is the size of the attributes (if present).
- Space: O(1) auxiliary space.

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

- **`nfs_mamont::serializer::option`**:
  - Mechanism: Serializes `Option` types.
  - Importance: Handles the conditional encoding of `symlink_attr`. It ensures the XDR "discriminant + value" pattern is correctly applied for optional fields.

- **`nfs_mamont::serializer::files::file_attr`**:
  - Mechanism: Serializes `file::Attr` structures.
  - Importance: Encodes the detailed file attributes (mode, size, timestamps, etc.) required by the NFS protocol for the `post_op_attr` field.

- **`nfs_mamont::serializer::files::file_path`**:
  - Mechanism: Serializes `file::Path` structures.
  - Importance: Encodes the actual string/byte content of the symbolic link, which is the primary data returned by the `READLINK` procedure.

---

## 4. Data Model

**Entities:**
- **`read_link::Success`**: A container for the successful result of a read link operation.
  - `symlink_attr`: `Option<file::Attr>` (Post-operation attributes).
  - `data`: `file::Path` (The target path).
- **`read_link::Fail`**: A container for the failed result.
  - `symlink_attr`: `Option<file::Attr>` (Post-operation attributes, even on failure).
  - `error`: `vfs::Error` (The specific error code, not serialized here).

**Relations:**
- `Success` → `file::Attr` (0..1)
- `Success` → `file::Path` (1)
- `Fail` → `file::Attr` (0..1)

**Global Invariants:**
- The `data` field in `Success` must be non-empty and valid according to the file system constraints, though the serializer treats it as an opaque byte/string sequence.
- The `error` field in `Fail` is ignored by this serializer; it is an invariant of the NFSv3 protocol that the error status is serialized in the union header, not the fail struct body.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Represents any I/O error occurring during the write process (e.g., disk full, broken pipe).

**Error Propagation Strategy:**
- Propagation via the `?` operator. If any underlying serialization call (`option`, `file_attr`, `file_path`) returns an `Err`, it is immediately returned to the caller.

**Recoverability:**
- Not recoverable within this module. The caller must handle the I/O error (e.g., by closing the connection or aborting the RPC request).

**Panics:**
- Allowed: No.
- Conditions: This module performs no explicit panics. Panics would only arise from bugs in the underlying `Write` implementation or logic errors in the dependency serializers.

---

## 6. Traits

- **`std::io::Write`**: This module does not implement the trait but requires it as a bound (`&mut impl Write`) for the destination argument.

---

## 7. Overview

This module is used in order to **convert high-level Virtual File System (VFS) results for the `READLINK` operation into the standardized External Data Representation (XDR) format required by the NFSv3 network protocol**.

The system contains a layered architecture where the **VFS layer** handles the logic of file system operations (like resolving symbolic links) and returns Rust structs representing success or failure. The **Serializer layer** (this module) acts as a translator, converting these Rust structs into a byte stream that can be sent over the network to an NFS client.

A typical usage scenario of the system involves:
1. The server receiving an NFSv3 `READLINK` request.
2. The VFS layer processing the request and returning a `Result<read_link::Success, read_link::Fail>`.
3. The RPC layer determining the status code (NFS3_OK vs error).
4. This module (`read_link` serializer) being invoked to serialize the body of the response (either `result_ok` or `result_fail`) into the output buffer.

Inside the system the following things happen and they use:
- **XDR Primitives**: The `option` function from the generic serializer is used to handle the optional nature of file attributes in the response.
- **File System Serializers**: The `file_attr` and `file_path` functions are used to ensure that complex file system data types are encoded according to the specific layout rules of NFSv3.
- **Protocol Compliance**: The separation of `result_ok` (serializing data + attributes) and `result_fail` (serializing only attributes) strictly follows the NFSv3 RFC 1813 specification for the `READLINK3res` union.