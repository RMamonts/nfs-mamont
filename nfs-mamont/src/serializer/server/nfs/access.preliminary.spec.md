<!-- SPEC_HASH: 23b6fca6507913c9fe1170d179e0f87f7d1d9e0ef20148ebd47217de05e88b85 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::access
Rust File: src/serializer/server/nfs/access.rs

---

## 1. Dependencies

- **`std::io::Write`**: Used as the trait bound for the destination buffer `dest`. It allows the serializer to write bytes into any target that implements the `write` method (e.g., `TcpStream`, `Vec<u8>`).
- **`crate::serializer::files::file_attr`**: Used to serialize the `file::Attr` structure. This is necessary because both `ACCESS3resok` and `ACCESS3resfail` contain `post_op_attr` (optional file attributes), which require the full attribute serialization logic.
- **`crate::serializer::option`**: Used to serialize the `Option<file::Attr>` fields (`object_attr`). In XDR, optional values are represented as a boolean discriminator followed by the value if present. This helper encapsulates that logic.
- **`crate::serializer::u32`**: Used to serialize the `access` mask in the success case. The access rights are a 32-bit bitmask defined by the NFSv3 protocol.
- **`crate::vfs::access`**: Provides the domain-specific types `access::Success` and `access::Fail`. These structs contain the data resulting from the VFS access check operation that needs to be converted to the wire format.

---

## 2. Mechanics

### Mechanism 1: Serialization of ACCESS3resok (`result_ok`)

**Intent:**
To convert the internal representation of a successful NFSv3 ACCESS procedure result (`access::Success`) into the XDR format defined by RFC 1813. This involves encoding the post-operation attributes and the granted access mask.

**Inputs:**
- `dest`: A mutable reference to a writer implementing `std::io::Write`.
- `arg`: An instance of `access::Success`, containing:
  - `object_attr`: An optional `file::Attr` representing the attributes of the object after the access check.
  - `access`: A `Mask` object representing the access rights granted to the user.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations to `dest`.

**Steps:**
1. Serialize the `object_attr` field using the `option` helper.
   - If `Some(attr)`, write a boolean discriminator (true) followed by the result of `file_attr(dest, &attr)`.
   - If `None`, write a boolean discriminator (false).
2. Serialize the `access` mask by calling `u32(dest, arg.access.bits())`. This converts the internal `Mask` type into a raw 32-bit integer and writes it to the stream.

**Edge Cases:**
- If `object_attr` is `None`, the `option` helper ensures only the "false" discriminator is written, omitting the attribute data.
- If the underlying `dest` writer returns an error (e.g., buffer full, broken pipe), the function propagates this `io::Error` immediately.

**Complexity:**
- Time: O(1), as the size of the data is bounded and fixed (attribute size is constant, mask is 4 bytes).
- Space: O(1), no additional heap allocation is performed by this function.

**Determinism:**
- Deterministic. Given the same input `arg` and a functioning `dest`, the sequence of bytes written is identical.

### Mechanism 2: Serialization of ACCESS3resfail (`result_fail`)

**Intent:**
To convert the internal representation of a failed NFSv3 ACCESS procedure result (`access::Fail`) into the XDR format. This specifically handles the `ACCESS3resfail` body, which only contains post-operation attributes.

**Inputs:**
- `dest`: A mutable reference to a writer implementing `std::io::Write`.
- `arg`: An instance of `access::Fail`, containing:
  - `error`: The specific `vfs::Error` that occurred.
  - `object_attr`: An optional `file::Attr`.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the `object_attr` field using the `option` helper, identical to the success case.
2. **Explicitly ignore** the `arg.error` field.

**Edge Cases:**
- The `arg.error` field is present in the input struct but is not serialized. This is consistent with the NFSv3 XDR specification, where the error status (`nfsstat3`) is part of the outer union header, not the `resfail` body itself.

**Complexity:**
- Time: O(1).
- Space: O(1).

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

- **`crate::serializer::option`**:
  - Mechanism: Encodes an `Option<T>` as a boolean followed by `T`.
  - Importance: Handles the `post_op_attr` (present in both success and fail responses) which is optional in the protocol.
- **`crate::serializer::u32`**:
  - Mechanism: Encodes a `u32` into big-endian bytes (XDR standard).
  - Importance: Used to transmit the `access` bitmask in the success response.
- **`crate::serializer::files::file_attr`**:
  - Mechanism: Encodes the `fattr3` structure (type, mode, nlink, uid, gid, size, etc.).
  - Importance: Provides the actual payload for the `object_attr` field when it is present.

---

## 4. Data Model

**Entities:**
- **`access::Success`**: A VFS entity representing a successful access check. It holds the resulting access mask and optional file attributes.
- **`access::Fail`**: A VFS entity representing a failed access check. It holds the error code and optional file attributes.
- **`access::Mask`**: A bitset entity representing specific access permissions (read, write, lookup, etc.).

**Relations:**
- `access::Success` contains `access::Mask`.
- Both `access::Success` and `access::Fail` contain `Option<file::Attr>`.

**Global Invariants:**
- The output byte stream must conform to the XDR encoding rules defined in RFC 1813 for structures `ACCESS3resok` and `ACCESS3resfail`.
- The `access` mask bits must be serialized as a 32-bit unsigned integer.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Represents failures during the write operation (e.g., disk full, network failure).

**Error Propagation Strategy:**
- Direct propagation. Any `Err` returned by `dest.write` or the helper serializers (`option`, `u32`, `file_attr`) is immediately returned to the caller via the `?` operator.

**Recoverability:**
- Not recoverable within this module. If a write fails, the serialization process aborts, and the error is returned to the upper layer (likely the RPC handler) to manage the connection state.

**Panics:**
- Allowed: No.
- Conditions: The code does not explicitly panic. Panics would only occur if the underlying `Write` implementation panics or if the helper serializers panic (which is generally unexpected for valid inputs).

---

## 6. Traits

This module does not implement any public traits. It acts as a consumer of the `std::io::Write` trait.

---

## 7. Overview

This module is used in order to **serialize the results of the NFSv3 ACCESS procedure into the XDR wire format**.

The system contains a **Virtual File System (VFS) layer** that performs high-level file system operations, such as checking user permissions on a specific file or directory. The VFS layer produces Rust-native structs (`access::Success`, `access::Fail`) representing the outcome of these operations. To communicate these results over the network to an NFS client, these internal structs must be converted into a standardized byte stream format defined by the NFSv3 protocol (XDR).

A typical usage scenario of the system involves:
1. The VFS receives an `ACCESS` request and checks permissions.
2. The VFS returns a `Result<access::Success, access::Fail>`.
3. The server logic determines which serializer function to call based on the result.
4. If successful, `result_ok` is called to write the attributes and the access mask.
5. If failed, `result_fail` is called to write the attributes (if available). Note that the error code itself is handled by the outer RPC response wrapper, not by this specific serializer.

Inside the system the following things happen and they use **helper serializers** (`option`, `u32`, `file_attr`) to handle the specific encoding rules of XDR, such as big-endian integer representation and optional value discriminators. This module isolates the protocol-specific formatting logic from the generic VFS logic.