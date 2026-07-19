<!-- SPEC_HASH: ef90e4fe853630cd04678db44371ccbdd45c5b01d6ad9a9540e3759743dea057 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::create
Rust File: src/serializer/server/nfs/create.rs

---

## 1. Dependencies

- **`std::io::Write`**: This trait is used as the abstraction for the destination byte stream. It allows the serializer to write XDR data to buffers, sockets, or any other type that implements byte writing.
- **`crate::serializer::files`**: This module provides specific XDR serialization primitives for file system entities. It is used to serialize the file handle (`file_handle`), file attributes (`file_attr`), and Weak Cache Consistency data (`wcc_data`).
- **`crate::serializer::option`**: This module provides the logic to serialize `Option` types according to XDR rules (encoding the presence or absence of a value). It is used to serialize the optional file handle and attributes in the success response.
- **`crate::vfs::create`**: This module defines the domain-specific data structures representing the outcome of a VFS creation operation. Specifically, `create::Success` and `create::Fail` are the source types being serialized.

---

## 2. Mechanics

### Mechanism 1: Serialization of Successful Creation (`result_ok`)

**Intent:**
To convert a successful file creation result from the internal VFS representation into the NFSv3 `CREATE3resok` XDR format and write it to the provided stream.

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait.
- `arg`: An instance of `create::Success`, containing the optional file handle, optional attributes, and WCC data.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the `file` field (an `Option<file::Handle>`) using the `option` helper. If present, it invokes `file_handle` to write the handle bytes.
2. Serialize the `attr` field (an `Option<file::Attr>`) using the `option` helper. If present, it invokes `file_attr` to write the attribute bytes.
3. Serialize the `wcc_data` field using the `wcc_data` helper.

**Edge Cases:**
- If `arg.file` is `None`, the `option` serializer writes the XDR representation for "absent" (typically a boolean false or zero length).
- If `arg.attr` is `None`, the `option` serializer writes the XDR representation for "absent".

**Complexity:**
- Time: O(N), where N is the total size of the file handle, attributes, and WCC data being written.
- Space: O(1) auxiliary space (excluding the output buffer).

**Determinism:**
- Deterministic. Given the same input struct and a functioning `Write` implementation, the output byte sequence is identical.

### Mechanism 2: Serialization of Failed Creation (`result_fail`)

**Intent:**
To convert a failed file creation result from the internal VFS representation into the NFSv3 `CREATE3resfail` XDR format and write it to the provided stream.

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait.
- `arg`: An instance of `create::Fail`, containing the error code and WCC data.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the `wcc_data` field using the `wcc_data` helper.

**Edge Cases:**
- The `arg.error` field present in the `create::Fail` struct is explicitly ignored by this function. In the NFSv3 protocol, the error status (stat) is part of the union discriminant handled by the calling layer (the generic RPC response serializer), not the body of the fail structure.

**Complexity:**
- Time: O(N), where N is the size of the WCC data.
- Space: O(1) auxiliary space.

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

Since specifications for dependencies are not provided, the following assumptions are made based on the `*.facts.json` files:

- **`crate::serializer::option`**: This mechanism handles the XDR encoding for optional values. It likely writes a discriminant (e.g., a boolean or integer) indicating presence, followed by the value itself if present. This is critical for `result_ok` where file handles and attributes may be optional.
- **`crate::serializer::files::wcc_data`**: This mechanism serializes `WccData` (Weak Cache Consistency data). It likely handles the serialization of `before` (pre-operation attributes) and `after` (post-operation attributes) fields, which are themselves optional. This is vital for both success and failure cases to allow the client to validate its cache.
- **`crate::vfs::create::Success`**: This struct acts as a data container. Its mechanics are purely structural, holding the results of a file creation attempt.

---

## 4. Data Model

**Entities:**
- **`create::Success`**: Represents the successful outcome of a file creation request.
    - `file`: `Option<file::Handle>` - The handle of the created file.
    - `attr`: `Option<file::Attr>` - The attributes of the created file.
    - `wcc_data`: `vfs::WccData` - Cache consistency data for the parent directory.
- **`create::Fail`**: Represents the failed outcome of a file creation request.
    - `error`: `vfs::Error` - The specific error code (ignored in serialization body).
    - `wcc_data`: `vfs::WccData` - Cache consistency data for the parent directory.

**Relations:**
- `create::Success` → `vfs::WccData` (1:1)
- `create::Fail` → `vfs::WccData` (1:1)

**Global Invariants:**
- The serialized output must conform to RFC 1813 (NFS Version 3 Protocol Specification).
- In `result_fail`, the `error` field is not serialized into the stream, implying the caller handles the status code serialization separately.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Represents any I/O error that occurs during the write operation (e.g., buffer full, broken pipe).

**Error Propagation Strategy:**
- Propagation via the `?` operator. If any underlying serialization call (`option`, `file_handle`, `wcc_data`) returns an `Err`, the function immediately returns that error to the caller.

**Recoverability:**
- Not recoverable within this module. The module is a pure transformer; if writing fails, the operation cannot be completed locally.

**Panics:**
- Allowed: No explicit panics are introduced in this module.
- Conditions: Panics may only occur if the underlying `Write` implementation panics or if the dependency serializers panic.

---

## 6. Traits

This module does not implement any external traits. It consumes the `std::io::Write` trait.

---

## 7. Overview

This module is used in order to translate the high-level results of a Virtual File System (VFS) file creation operation into the low-level External Data Representation (XDR) format required by the NFSv3 network protocol. It serves as a specific adapter for the `CREATE` procedure response.

The system contains a layered architecture where the VFS layer handles the logic of file creation (checking permissions, allocating inodes, etc.) and returns Rust structs (`Success` or `Fail`). The networking layer, however, operates on streams of bytes conforming to the XDR standard. This module bridges these layers.

A typical usage scenario involves the server receiving a `CREATE` request. The VFS processes it and returns a `Result<Success, Fail>`. The RPC dispatcher determines which serializer function to call. If successful, `result_ok` is invoked to write the new file's handle and attributes to the network buffer, along with directory cache data. If the operation fails, `result_fail` is invoked to write only the directory cache data, allowing the client to update its cache despite the error.

Inside the system, the following things happen and they use:
1.  **Data Extraction**: The module extracts specific fields (`file`, `attr`, `wcc_data`) from the VFS result structs.
2.  **Format Conversion**: It uses helper serializers (`option`, `file_handle`, `wcc_data`) to convert these Rust types into their XDR byte representations.
3.  **Stream Writing**: It writes these bytes sequentially to the provided `Write` destination (usually a network buffer).

This separation ensures that the VFS logic remains independent of the wire protocol details, while the serializer handles the strict formatting requirements of NFSv3.