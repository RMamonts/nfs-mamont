<!-- SPEC_HASH: 91c5ee3439fc4d8b33f05371eb75f1e6cf21809469069dd680ad99a251cd1edf -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::rm_dir
Rust File: src/serializer/server/nfs/rm_dir.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**: Used to provide the `Write` trait, which defines the interface for the destination byte stream (e.g., a network buffer), and `io::Result` for handling I/O errors during serialization.
- **`crate::serializer::files`**: Used to access the `wcc_data` function. This function is responsible for the actual XDR encoding of Weak Cache Consistency data, which is the primary payload of the `RMDIR` response.
- **`crate::vfs::rm_dir`**: Used to import the `Success` and `Fail` types. These structs represent the result of the VFS `RMDIR` operation and contain the `WccData` fields that need to be serialized.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide specific serialization functions for the `RMDIR` NFSv3 procedure response bodies.
- To map the VFS result types (`Success` and `Fail`) to their corresponding XDR representations by extracting the relevant Weak Cache Consistency (WCC) data and delegating the low-level encoding to the generic `wcc_data` serializer.

Inputs:
- `dest`: A mutable reference to a type implementing `std::io::Write`.
- `arg`: Either `rm_dir::Success` (representing a successful removal) or `rm_dir::Fail` (representing a failed removal).

Outputs:
- `io::Result<()>`: Indicates success or failure in writing the serialized bytes to the destination.

Steps:
1. **Serialization of Success (`result_ok`)**:
 - The function receives `arg: rm_dir::Success`.
 - It extracts the `wcc_data` field from `arg`.
 - It calls `crate::serializer::files::wcc_data(dest, arg.wcc_data)` to write the post-operation attributes to the destination.
2. **Serialization of Failure (`result_fail`)**:
 - The function receives `arg: rm_dir::Fail`.
 - It extracts the `dir_wcc` field from `arg`.
 - It calls `crate::serializer::files::wcc_data(dest, arg.dir_wcc)` to write the pre-operation attributes to the destination.

Edge Cases:
- **None**: The module acts as a pass-through to `wcc_data`. Edge cases (e.g., invalid attribute data) are handled by the dependency.

Complexity:
- **Time**: O(N), where N is the size of the `WccData` structure being serialized. This complexity is inherited from the `wcc_data` function.
- **Space**: O(1) auxiliary space (excluding the buffer managed by the `Write` implementation).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer::files`**:
 - **`wcc_data`**: This is the core serialization mechanism used by this module. It handles the conversion of `vfs::WccData` (containing optional pre- and post-operation attributes) into the XDR `wcc_data` format. The current module relies on this to ensure that the specific WCC fields extracted from `rm_dir::Success` and `rm_dir::Fail` are correctly encoded according to the NFSv3 standard.

- **From `nfs_mamont::vfs::rm_dir`**:
 - **`Success` and `Fail` Structs**: These structures define the data contract for the `RMDIR` operation. The current module depends on the specific field names (`wcc_data` in `Success` and `dir_wcc` in `Fail`) to access the data intended for the wire. The distinction between these fields (post-op vs pre-op) is critical for the client's cache consistency logic.

---

## 4. Data Model

Entities:
- **`rm_dir::Success`**: A VFS result structure containing `wcc_data` (post-operation attributes).
- **`rm_dir::Fail`**: A VFS result structure containing `dir_wcc` (pre-operation attributes) and an error code.

Relations:
- **`result_ok` → `rm_dir::Success`**: The function consumes the success struct to access its WCC data.
- **`result_fail` → `rm_dir::Fail`**: The function consumes the fail struct to access its WCC data.

Global Invariants:
- The `wcc_data` field in `Success` must represent the state of the directory *after* the removal.
- The `dir_wcc` field in `Fail` must represent the state of the directory *before* the failed removal attempt.

## 5. Error Model

Error Types:
- **`std::io::Error`**: Propagated from the underlying `Write` implementation or from the `wcc_data` function.

Error Propagation Strategy:
- **Propagation**: Errors are propagated using the `?` operator. If `wcc_data` fails, the error is immediately returned to the caller.

Recoverability:
- **Recoverable**: The caller receives a `Result`, allowing it to handle I/O errors (e.g., buffer full, connection reset) appropriately.

Panics:
- **Allowed**: No. The code performs no operations that can panic (e.g., no indexing, no unwrapping).

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This module is used in order to serialize the response bodies for the NFSv3 `RMDIR` procedure. The system contains an NFSv3 server that processes file system requests. When a `RMDIR` request is executed, the Virtual File System (VFS) layer returns a result indicating success or failure. This result includes Weak Cache Consistency (WCC) data—attributes of the parent directory—to help the client validate its cache.

A typical usage scenario of the system involves the server successfully removing a directory. The VFS returns a `Success` struct containing the updated attributes of the parent directory. The RPC layer, responsible for constructing the network packet, calls `result_ok`. This function extracts the `wcc_data` and serializes it into the output buffer. Conversely, if the removal fails (e.g., directory not empty), the VFS returns a `Fail` struct. The RPC layer calls `result_fail`, which serializes the `dir_wcc` (attributes before the attempt) so the client can still update its cache state despite the error.

Inside the system, the following things happen and they use this module:
- **Response Construction**: The RPC dispatcher uses `result_ok` or `result_fail` to write the specific body of the `RMDIR3res` XDR union. The dispatcher handles the status code (discriminant), while this module handles the data associated with that status.
- **Cache Consistency**: By delegating to `wcc_data`, this module ensures that the complex logic of serializing optional pre- and post-attributes is reused, maintaining consistency across different NFS procedures that modify directory state.

This module is necessary because the `RMDIR` response structure in the NFSv3 protocol differs slightly in field naming and context compared to other procedures (e.g., `REMOVE`), even though the underlying WCC data structure is identical. This module provides the specific adapter functions that map the VFS result types to the generic serializer, ensuring the correct data is placed on the wire.