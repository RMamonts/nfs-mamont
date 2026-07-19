<!-- SPEC_HASH: 1dd43b3532cc51c76fc2b8fdc2803150c9fe86759e37819329a9b74fc89abf40 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::rename
Rust File: src/serializer/server/nfs/rename.rs

---

## 1. Dependencies

From the provided context and `*.facts.json` files, the module depends on the following:

- **`std::io` and `std::io::Write`**: Used for the output stream abstraction. The module writes bytes to any target implementing the `Write` trait (e.g., network sockets, buffers).
- **`crate::serializer::files`**: Specifically the `wcc_data` function. This dependency is used to handle the serialization of Weak Cache Consistency (WCC) data, which is a complex part of the NFSv3 protocol involving optional pre- and post-operation attributes.
- **`crate::vfs::rename`**: Provides the domain-specific types `rename::Success` and `rename::Fail`. These structures contain the data resulting from a VFS-level rename operation (specifically the WCC data for the source and target directories) that needs to be serialized.

---

## 2. Mechanics

This module is responsible for converting the result of a VFS rename operation into the XDR (External Data Representation) format required by the NFSv3 protocol. It handles both the success and failure cases of the `RENAME` procedure response.

Intent:
- To serialize the body of the NFSv3 `RENAME3res` response. Specifically, it encodes the Weak Cache Consistency (WCC) data for the source (`from`) and target (`to`) directories. This allows the client to update its cache without re-querying attributes.

Inputs:
- `dest`: A mutable reference to a writer implementing `std::io::Write`.
- `arg`: Either a `rename::Success` or `rename::Fail` structure, containing `from_dir_wcc` and `to_dir_wcc`.

Outputs:
- `io::Result<()>`: Indicates successful serialization or an I/O error.

Steps:
1.  **Serialize Source WCC**: Call `wcc_data` passing the destination writer and the `from_dir_wcc` field from the argument.
2.  **Serialize Target WCC**: Call `wcc_data` passing the destination writer and the `to_dir_wcc` field from the argument.
3.  Return the result of the operations.

Edge Cases:
- **Error Field Ignored**: In `result_fail`, the `arg.error` field present in the `rename::Fail` struct is *not* serialized by this function. This implies that the calling context handles the serialization of the status code (discriminator) and the specific error value before or after calling this function, or that this function corresponds strictly to the data payload following the status in the XDR union.

Complexity:
- Time: O(N) where N is the size of the WCC data being written (delegated to `wcc_data`).
- Space: O(1) auxiliary space (excluding the output buffer).

Determinism:
- Deterministic. Given the same input structures and writer, the sequence of bytes written is identical.

---

## 3. Dependency Mechanics

Since full specifications for dependencies are not provided, the following mechanics are inferred from the `*.facts.json` files:

- **`crate::serializer::files::wcc_data`**:
    - This function is the core mechanism used by the current module.
    - It accepts a `vfs::WccData` structure.
    - `vfs::WccData` contains `before: Option<file::WccAttr>` and `after: Option<file::Attr>`.
    - It is assumed that `wcc_data` handles the XDR encoding of these optional fields, likely using the `option` helper from `crate::serializer` to encode the presence/absence of attributes followed by the attribute data itself.

- **`crate::vfs::rename::Success` and `crate::vfs::rename::Fail`**:
    - These are data structures (structs) defined in the VFS layer.
    - They act as carriers for the result of the rename operation.
    - Both structs contain `from_dir_wcc` and `to_dir_wcc` fields of type `vfs::WccData`.
    - The `Fail` struct additionally contains an `error: vfs::Error` field, which is ignored by the serializer in this module (as noted in Edge Cases).

---

## 4. Data Model

Entities:
- **`rename::Success`**: Represents the successful outcome of a rename operation. It holds WCC data for the source and target directories to allow cache validation.
- **`rename::Fail`**: Represents the failed outcome of a rename operation. It holds an error code and WCC data for the source and target directories.

Relations:
- The module does not define relationships but consumes the fields of the entities mentioned above.

Global Invariants:
- **Serialization Order**: The WCC data for the source directory (`from_dir_wcc`) must always be serialized before the WCC data for the target directory (`to_dir_wcc`). This order is mandated by the NFSv3 RFC 1813 specification for the `RENAME3res` structure.

---

## 5. Error Model

Error Types:
- **`std::io::Error`**: The only error type explicitly returned by the public functions. This typically indicates a failure to write to the underlying stream (e.g., broken pipe, buffer full).

Error Propagation Strategy:
- Propagation is direct. Errors returned by `wcc_data` or the underlying `Write` implementation are propagated immediately to the caller using the `?` operator.

Recoverability:
- Serialization errors are generally considered unrecoverable at the protocol handling level. If a response cannot be written to the client, the RPC request usually fails.

Panics:
- Allowed: No.
- Conditions: The code does not explicitly panic. It relies on safe Rust mechanisms.

---

## 6. Traits

List which external traits this module implements:
- None. The module provides free-standing functions.

List which external traits this module uses:
- **`std::io::Write`**: Used as a trait bound (`&mut impl Write`) for the destination buffer.

---

## 7. Overview

This module is used in order to **implement the server-side response serialization for the NFSv3 RENAME procedure**.

This system contains **a layered architecture where the Virtual File System (VFS) handles the logic of file operations (renaming), and the Serializer layer handles the translation of internal VFS state into the standardized XDR wire format**.

A typical usage scenario of the system involves the server receiving an RPC request to rename a file. The VFS layer processes this request, performing checks and filesystem modifications. It returns a result type (`Result<Success, Fail>`) which includes metadata about the directories involved (WCC data). The server logic then invokes either `result_ok` or `result_fail` from this module. These functions take the VFS result structures and write the appropriate sequence of bytes to the network stream, ensuring the client receives the correct cache consistency information regardless of whether the rename succeeded or failed.

Inside the system the following things happen and they use **the `wcc_data` helper to encode optional file attributes**. The module effectively decouples the protocol formatting logic from the filesystem logic, allowing the VFS to deal with high-level concepts like `WccData` while the serializer handles the bitwise representation required by the NFS protocol.

**Assumptions:**
- It is assumed that the caller of `result_fail` has already serialized or will serialize the `error` field found in `rename::Fail`, as this module explicitly ignores it.
- It is assumed that `wcc_data` correctly implements the XDR standard for encoding optional attributes.