<!-- SPEC_HASH: 7ddc122121adb4241b77652704ee0423d0dd952140d16b326ba1ee835311fb80 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::remove
Rust File: src/serializer/server/nfs/remove.rs

---

## 1. Dependencies

- **`std::io` / `std::io::Write`**
  - **Purpose:** Provides the `Write` trait, which is the abstraction used for the output destination (e.g., a TCP stream or a memory buffer). The module uses this to write the serialized XDR bytes.
- **`crate::vfs::remove`**
  - **Purpose:** Supplies the domain-specific data structures representing the outcome of a VFS remove operation: `remove::Success` and `remove::Fail`. These structures contain the Weak Cache Consistency (WCC) data that must be serialized.
- **`crate::serializer::files`**
  - **Purpose:** Provides the `wcc_data` helper function. This module delegates the actual encoding logic for `WccData` to this dependency to avoid code duplication, as WCC data is common across many NFS procedures.

---

## 2. Mechanics

**Intent:**
To serialize the result of an NFSv3 `REMOVE` procedure (both success and failure cases) into the XDR format. The module acts as an adapter, extracting the Weak Cache Consistency (WCC) data from the VFS result types and passing them to the generic WCC serializer.

**Inputs:**
- `dest`: A mutable reference to a type implementing the `Write` trait.
- `arg`: Either a `remove::Success` or `remove::Fail` structure, containing the WCC data.

**Outputs:**
- `io::Result<()>`: Indicates successful completion of the write operation or propagates an I/O error.
- **Side Effect:** Writes a sequence of bytes conforming to the NFSv3 XDR specification for `REMOVE3resok` or `REMOVE3resfail` into `dest`.

**Steps:**
1.  **Receive Result:** The function accepts either a `Success` or `Fail` struct from the `vfs::remove` module.
2.  **Extract WCC Data:**
    - For `result_ok`, the `wcc_data` field is extracted.
    - For `result_fail`, the `dir_wcc` field is extracted.
3.  **Delegate Serialization:** The extracted `vfs::WccData` is passed to the `crate::serializer::files::wcc_data` function along with the `dest` writer.
4.  **Return:** The `io::Result` returned by the `wcc_data` function is propagated to the caller.

**Edge Cases:**
- **I/O Errors:** If the underlying `dest` writer fails (e.g., buffer full, connection reset), the error is immediately returned.
- **Invalid Data:** The module assumes the `WccData` provided by the VFS layer is valid. It does not perform logical validation of the file system state, only structural serialization.

**Complexity:**
- **Time:** O(N), where N is the size of the serialized `WccData`. This is determined by the `wcc_data` dependency.
- **Space:** O(1) auxiliary space (excluding the output buffer managed by `dest`).

**Determinism:**
- Deterministic. Given the same input struct and a `Write` implementation that behaves deterministically, the output byte sequence is identical.

---

## 3. Dependency Mechanics

- **`crate::serializer::files::wcc_data`**:
  - This is the primary mechanism used. It serializes a `vfs::WccData` struct (containing optional pre-operation `WccAttr` and post-operation `Attr`) into XDR format. It handles the encoding of optional fields and the specific attribute structures required by the NFSv3 protocol.
- **`crate::vfs::remove::Success` / `crate::vfs::remove::Fail`**:
  - These structs serve as data containers. The critical mechanic is their field access: `Success.wcc_data` and `Fail.dir_wcc`. Both fields are of type `vfs::WccData`, allowing the current module to treat them uniformly for serialization purposes, despite their semantic difference in the VFS layer.

---

## 4. Data Model

**Entities:**
- **`remove::Success`**: Represents the successful result of a file removal operation.
  - Contains `wcc_data: vfs::WccData`.
- **`remove::Fail`**: Represents the failed result of a file removal operation.
  - Contains `dir_wcc: vfs::WccData`.
- **`vfs::WccData`**: Weak Cache Consistency data.
  - Contains `before: Option<file::WccAttr>` (attributes before the operation).
  - Contains `after: Option<file::Attr>` (attributes after the operation).

**Relations:**
- `remove::Success` → `vfs::WccData` (1:1 composition)
- `remove::Fail` → `vfs::WccData` (1:1 composition)

**Global Invariants:**
- The XDR output for `REMOVE3resok` and `REMOVE3resfail` consists exclusively of the `wcc_data` structure. There are no additional fields in the response body defined by the NFSv3 RFC 1813 for this procedure.

---

## 5. Error Model

**Error Types:**
- `std::io::Error`

**Error Propagation Strategy:**
- Direct propagation. The module does not define custom error types. Any error encountered during writing (originating from the `Write` implementation or the `wcc_data` helper) is returned immediately via the `?` operator.

**Recoverability:**
- Recoverability is handled by the caller. If an `Err` is returned, the state of the `dest` writer is undefined (partially written), and the RPC response handling layer must typically abort or reset the connection/stream.

**Panics:**
- **Allowed:** No.
- **Conditions:** This module performs no allocation or complex logic that should panic. Panics would only arise from bugs in the `wcc_data` dependency or the `Write` implementation.

---

## 6. Traits

This module does not implement any external traits. It uses the `std::io::Write` trait as a constraint on its `dest` argument.

---

## 7. Overview

This module is used in order to **convert the internal result of a file system remove operation into the network protocol format (XDR) required by the NFSv3 standard**.

This system contains **a serialization layer that sits between the VFS (Virtual File System) logic and the network transport**. The VFS layer produces Rust structs representing the outcome of operations (like `remove::Success`), while the network layer requires a specific byte stream defined by RFC 1813.

A typical usage scenario of the system involves the server handling an NFS `REMOVE` request. After the VFS layer attempts to delete the file, it returns a `Result<Success, Fail>`. The server logic invokes either `result_ok` or `result_fail` from this module. The module extracts the directory's Weak Cache Consistency (WCC) data—which allows the client to update its cache without re-reading the directory if it hasn't changed—and serializes it into the response buffer.

Inside the system the following things happen and they use **delegation to a shared `wcc_data` serializer**. Since the NFSv3 `REMOVE` response body (both `REMOVE3resok` and `REMOVE3resfail`) contains only `wcc_data`, this module acts as a thin wrapper that maps the specific VFS result types to the generic serializer, ensuring the correct on-wire representation is generated.