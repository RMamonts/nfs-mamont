<!-- SPEC_HASH: a937c9de0c308bf87fa47754386757107d85435a4f299e0e739b01c5d76d0b89 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::symlink
Rust File: src/serializer/server/nfs/symlink.rs

---

## 1. Dependencies

- **`std::io` / `std::io::Write`**
  - **Purpose**: Provides the `Write` trait, which is the abstraction used for the output byte stream. This allows the serializer to write to buffers, network sockets, or files without knowing the concrete destination type.
- **`crate::serializer::files`**
  - **Purpose**: Supplies specific XDR serialization primitives for file system entities. Specifically, `file_handle`, `file_attr`, and `wcc_data` are used to encode the components of the NFS response structure.
- **`crate::serializer::option`**
  - **Purpose**: Provides a generic helper to serialize `Option<T>` types according to XDR rules (encoding a boolean discriminator followed by the value if present).
- **`crate::vfs::symlink`**
  - **Purpose**: Defines the input data structures (`symlink::Success` and `symlink::Fail`) that represent the logical outcome of the VFS `SYMLINK` operation. This module converts these high-level representations into the wire format.

---

## 2. Mechanics

### Mechanism 1: Serialization of Successful SYMLINK Response (`result_ok`)

**Intent:**
To convert a successful VFS `SYMLINK` operation result into the XDR format defined for `SYMLINK3resok` (RFC 1813). This involves encoding the new file handle, its attributes, and directory cache consistency data.

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait (the output buffer).
- `arg`: A `symlink::Success` struct containing:
  - `file`: An optional file handle for the created symlink.
  - `attr`: Optional attributes for the created symlink.
  - `wcc_data`: Weak cache consistency data for the directory.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the optional file handle (`arg.file`) using the `option` helper. If `Some`, it invokes `file_handle`; otherwise, it writes the XDR representation for `None`.
2. Serialize the optional file attributes (`arg.attr`) using the `option` helper. If `Some`, it invokes `file_attr`; otherwise, it writes the XDR representation for `None`.
3. Serialize the Weak Cache Consistency data (`arg.wcc_data`) by invoking `wcc_data`.

**Edge Cases:**
- If `arg.file` or `arg.attr` are `None`, the `option` function handles the encoding of the absence (typically a boolean false).
- If the underlying `Write` implementation fails (e.g., buffer full, I/O error), the error is propagated immediately.

**Complexity:**
- **Time:** O(1) relative to logical operations (fixed number of fields), though dependent on the size of the file handle and attributes being written.
- **Space:** O(1) auxiliary space; writes directly to the provided buffer.

**Determinism:**
- Deterministic. Given the same input struct and a functioning `Write` implementation, the output byte sequence is identical.

### Mechanism 2: Serialization of Failed SYMLINK Response (`result_fail`)

**Intent:**
To convert a failed VFS `SYMLINK` operation result into the XDR format defined for `SYMLINK3resfail` (RFC 1813). This focuses on encoding the directory state after the failed attempt to maintain cache consistency.

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait.
- `arg`: A `symlink::Fail` struct containing:
  - `dir_wcc`: Weak cache consistency data for the directory.

**Outputs:**
- `io::Result<()>`: Indicates success or failure of the write operations.

**Steps:**
1. Serialize the directory Weak Cache Consistency data (`arg.dir_wcc`) by invoking `wcc_data`.

**Edge Cases:**
- The `symlink::Fail` struct contains an `error` field (`vfs::Error`), but this function **does not** serialize it. This implies the NFS status code is serialized by the caller before invoking this function.

**Complexity:**
- **Time:** O(1).
- **Space:** O(1).

**Determinism:**
- Deterministic.

---

## 3. Dependency Mechanics

- **From `nfs_mamont::serializer::files`**:
  - `file_handle`: Encodes the opaque file identifier bytes.
  - `file_attr`: Encodes the `fattr3` structure (type, mode, size, etc.).
  - `wcc_data`: Encodes the `wcc_data` structure, which consists of a `pre_op_attr` (optional) and `post_op_attr` (optional).
- **From `nfs_mamont::serializer`**:
  - `option`: Encodes an XDR optional variable. It writes a boolean (true if value present, false otherwise) and conditionally calls the provided closure to serialize the inner value.
- **From `nfs_mamont::vfs::symlink`**:
  - `Success` / `Fail` structures: These are the source data types. `Success` holds the result of a successful creation, while `Fail` holds the directory state after a failure.

---

## 4. Data Model

**Entities:**
- **`symlink::Success`**: A data transfer object (DTO) representing a successful symlink creation.
  - `file`: `Option<file::Handle>` (The handle of the new symlink).
  - `attr`: `Option<file::Attr>` (The attributes of the new symlink).
  - `wcc_data`: `vfs::WccData` (Directory state before/after).
- **`symlink::Fail`**: A DTO representing a failed symlink creation.
  - `dir_wcc`: `vfs::WccData` (Directory state before/after).

**Relations:**
- `symlink::Success` → `vfs::WccData` (1:1)
- `symlink::Fail` → `vfs::WccData` (1:1)

**Global Invariants:**
- The output byte stream must conform to the XDR definition of `SYMLINK3resok` and `SYMLINK3resfail` as specified in RFC 1813.
- The `result_fail` function assumes the NFS status code has been or will be written by the caller, as it only serializes the `dir_wcc` field of the union body.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`: Represents I/O errors encountered during writing (e.g., broken pipe, insufficient buffer space).

**Error Propagation Strategy:**
- Propagation via the `?` operator. Any error returned by `option`, `file_handle`, `file_attr`, or `wcc_data` is immediately returned to the caller.

**Recoverability:**
- Not recoverable within this module. If a write fails, the serialization process aborts, and the error is returned to the upper layer (likely the RPC handler) to close the connection or discard the response.

**Panics:**
- **Allowed:** No.
- **Conditions:** This module performs no explicit panics. Panics would only arise from bugs in dependency implementations or `Write` trait violations.

---

## 6. Traits

- **`std::io::Write`**: Used by the public functions (`result_ok`, `result_fail`) as a trait bound (`&mut impl Write`) to accept any byte stream destination.

---

## 7. Overview

This module is used in order to **serialize the results of the NFSv3 SYMLINK procedure into the XDR (External Data Representation) format required for network transmission**.

This system contains **a set of serializers that translate high-level Virtual File System (VFS) operation results into protocol-specific byte streams**. The `SYMLINK` procedure allows a client to create a symbolic link on the server.

A typical usage scenario of the system involves the server receiving an RPC request for `SYMLINK`, executing the logic via the `Vfs` trait, and obtaining a `Result<symlink::Success, symlink::Fail>`. The server must then send a response back to the client. This module is invoked to format the "ok" or "fail" body of that response.

Inside the system the following things happen and they use **the `Write` trait to output bytes, `option` to handle optional XDR fields, and specific file serializers (`file_handle`, `file_attr`, `wcc_data`) to encode complex structures**:
1. If the operation succeeded, `result_ok` writes the new file handle (if available), the new attributes (if available), and the directory's Weak Cache Consistency (WCC) data.
2. If the operation failed, `result_fail` writes only the directory's WCC data. Note that the specific NFS error code (e.g., `NFS3ERR_ACCES`) is not serialized by this module; it is assumed to be handled by the generic RPC framing layer which calls this module for the union body.

**Assumptions:**
- The caller handles the serialization of the NFS status integer (the discriminant of the result union) before calling these functions.
- The `symlink::Success` and `symlink::Fail` structures are populated correctly by the VFS layer.
- The `serializer::files` module correctly implements the XDR standard for file attributes and handles.