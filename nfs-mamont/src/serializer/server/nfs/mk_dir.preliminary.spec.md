<!-- SPEC_HASH: 7a74a19a31e90044fe86bc570232541447eea4fd4c438aa95577238eb7e3468d -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::mk_dir
Rust File: src/serializer/server/nfs/mk_dir.rs

---

## 1. Dependencies

- **`std::io` and `std::io::Write`**: Used for the output stream abstraction. The module writes bytes to any target implementing the `Write` trait (e.g., network sockets, buffers).
- **`crate::serializer::files`**: Provides specific XDR serialization primitives for file system entities. Specifically, `file_handle`, `file_attr`, and `wcc_data` are used to encode the complex types returned by the VFS layer.
- **`crate::serializer::option`**: Provides the XDR standard mechanism for serializing optional values (a boolean discriminant followed by the value if present). This is used for the file handle and attributes in the success case.
- **`crate::vfs::mk_dir`**: Defines the input data structures (`Success` and `Fail`) which represent the logical outcome of a directory creation operation within the Virtual File System.

---

## 2. Mechanics

### Mechanism 1: Serialization of MKDIR3resok (`result_ok`)

**Intent:**
To convert a successful directory creation result from the VFS layer into the NFSv3 XDR wire format. This involves encoding the new directory's handle, its attributes, and the Weak Cache Consistency (WCC) data for the parent directory.

**Inputs:**
- `dest`: A mutable reference to a writer implementing `std::io::Write`.
- `arg`: An instance of `mk_dir::Success`, containing:
  - `file`: An optional file handle for the new directory.
  - `attr`: Optional attributes for the new directory.
  - `wcc_data`: `vfs::WccData` containing pre- and post-operation attributes of the parent directory.

**Outputs:**
- A sequence of bytes written to `dest` conforming to the `MKDIR3resok` structure in RFC 1813.
- `io::Result<()>` indicating success or failure of the write operation.

**Steps:**
1. Serialize the optional file handle (`arg.file`) using the `option` helper. If present, it delegates to `file_handle`.
2. Serialize the optional file attributes (`arg.attr`) using the `option` helper. If present, it delegates to `file_attr`.
3. Serialize the WCC data (`arg.wcc_data`) by delegating to `wcc_data`.

**Edge Cases:**
- If `arg.file` or `arg.attr` are `None`, the `option` serializer writes a `0` (false) discriminant and skips the data serialization, adhering to XDR variable-length/optional encoding rules.

**Complexity:**
- Time: O(N), where N is the total size of the serialized fields (constant time relative to logic, linear relative to data size).
- Space: O(1) auxiliary space (excluding the output buffer).

**Determinism:**
- Deterministic. The same input struct produces the exact same byte sequence.

### Mechanism 2: Serialization of MKDIR3resfail (`result_fail`)

**Intent:**
To convert a failed directory creation result from the VFS layer into the NFSv3 XDR wire format. This focuses on providing cache consistency data for the parent directory despite the operation failing.

**Inputs:**
- `dest`: A mutable reference to a writer implementing `std::io::Write`.
- `arg`: An instance of `mk_dir::Fail`, containing:
  - `error`: The specific `vfs::Error` that occurred.
  - `dir_wcc`: `vfs::WccData` for the parent directory.

**Outputs:**
- A sequence of bytes written to `dest` conforming to the `MKDIR3resfail` structure in RFC 1813.
- `io::Result<()>` indicating success or failure of the write operation.

**Steps:**
1. Serialize the WCC data (`arg.dir_wcc`) by delegating to `wcc_data`.

**Edge Cases:**
- The `error` field within `arg` is explicitly **not** serialized by this function. This implies the NFS status code (union discriminant) is serialized by the caller (e.g., a generic RPC response serializer) before invoking this function to serialize the union body.

**Complexity:**
- Time: O(N), where N is the size of the WCC data.
- Space: O(1) auxiliary space.

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

- **`crate::serializer::files::wcc_data`**: Encodes the `WccData` struct. This is critical for NFS cache consistency, allowing the client to validate its cache of the parent directory attributes before and after the operation.
- **`crate::serializer::files::file_handle`**: Encodes the opaque identifier for the newly created directory.
- **`crate::serializer::files::file_attr`**: Encodes the standard file attributes (mode, size, timestamps, etc.) for the new directory.
- **`crate::serializer::option`**: Handles the XDR encoding for optional fields. In NFSv3, file handles and attributes in the `MKDIR3resok` are optional to allow the server to return them if available, or omit them if the cost of retrieval is too high (though usually required for `MKDIR`).

---

## 4. Data Model

**Entities:**
- **`mk_dir::Success`**: Represents the successful outcome of a directory creation.
  - Contains the new directory's identity (`file`) and state (`attr`).
  - Contains the parent directory's state change information (`wcc_data`).
- **`mk_dir::Fail`**: Represents the failed outcome.
  - Contains the parent directory's state change information (`dir_wcc`).
  - Contains the error code (`error`), though this is not serialized by this specific module.

**Relations:**
- `Success` → `WccData` (1:1): Associates the success with the parent directory's state.
- `Fail` → `WccData` (1:1): Associates the failure with the parent directory's state.

**Global Invariants:**
- The module assumes that the input structs (`Success`, `Fail`) are valid according to the VFS layer logic. It performs no logical validation, only structural serialization.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Represents low-level I/O errors during writing (e.g., buffer full, broken pipe).

**Error Propagation Strategy:**
- Propagation: The `?` operator is used to propagate errors immediately from the underlying serialization primitives (`option`, `wcc_data`, etc.) to the caller.
- Type: Standard `io::Result`.

**Recoverability:**
- Serialization errors are generally considered fatal for the specific RPC request being processed. The module does not implement retry logic or partial recovery.

**Panics:**
- Allowed: No explicit panics in the code.
- Conditions: Panics may originate from the `Write` implementation or the dependency serializers if invariants are violated (e.g., logic errors in dependencies).

---

## 6. Traits

- **`std::io::Write`**: Used by the public functions (`result_ok`, `result_fail`) as a trait bound (`&mut impl Write`) to accept any byte stream target.

---

## 7. Overview

This module is used in order to **serialize the response payload for the NFSv3 `MKDIR` procedure**. It acts as a bridge between the high-level Virtual File System (VFS) results and the low-level network byte stream required by the NFS protocol.

The system contains a layered architecture where the **VFS layer** handles the actual logic of creating directories and returns Rust structs (`Success` or `Fail`). The **Serializer layer** (this module) is responsible for converting these structs into the standardized XDR format defined in RFC 1813.

A typical usage scenario involves the server receiving an `MKDIR` request, dispatching it to the VFS, receiving a `Result<Success, Fail>`, and then invoking the appropriate function from this module (`result_ok` or `result_fail`) to write the response body to the network socket.

Inside the system, the following things happen and they use:
1.  **Result Discrimination**: The caller determines if the operation succeeded or failed.
2.  **Status Serialization**: The caller serializes the NFS status code (integer).
3.  **Payload Serialization**: This module is invoked to serialize the specific payload (either `resok` or `resfail`).
    -   For `resok`, it uses `option` to serialize the new handle and attributes, followed by `wcc_data` for the parent.
    -   For `resfail`, it serializes `wcc_data` for the parent to allow the client to update its cache even if the operation failed.

This separation ensures that the protocol encoding details are encapsulated within the serializer module, keeping the VFS logic independent of network formats.