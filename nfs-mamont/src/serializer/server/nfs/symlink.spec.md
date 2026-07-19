<!-- SPEC_HASH: a937c9de0c308bf87fa47754386757107d85435a4f299e0e739b01c5d76d0b89 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::symlink
Rust File: src/serializer/server/nfs/symlink.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io` and `std::io::Write`**: Used to define the interface for the destination byte stream (`dest`). The `Write` trait allows the serializer to write bytes into a buffer (e.g., a network socket or memory buffer) in an abstracted way.
- **`crate::serializer::files`**: Used to access specific serializers for VFS types. Specifically, `file_handle` is used to serialize the handle of the created symlink, `file_attr` is used to serialize the attributes of the symlink, and `wcc_data` is used to serialize the Weak Cache Consistency data for the directory.
- **`crate::serializer::option`**: Used to serialize optional fields (`Option<T>`). In the context of NFSv3 `SYMLINK3resok`, the file handle and attributes are technically optional unions (though usually present), and this helper handles the XDR discriminant (boolean) and the value serialization.
- **`crate::vfs::symlink`**: Used as the source of the data structures to be serialized. The `symlink::Success` and `symlink::Fail` structs contain the data that results from the VFS operation, which this module transforms into the wire format.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide XDR serialization functions for the results of the NFSv3 `SYMLINK` procedure.
- To map the internal VFS result types (`symlink::Success` and `symlink::Fail`) to the binary format defined in RFC 1813 (`SYMLINK3resok` and `SYMLINK3resfail`).

Inputs:
- `dest`: A mutable reference to a type implementing `std::io::Write`, representing the destination buffer for the XDR bytes.
- `arg`: Either a `symlink::Success` struct (for successful operations) or a `symlink::Fail` struct (for failed operations).

Outputs:
- `io::Result<()>`: Indicates success if all bytes were written, or an `io::Error` if a write operation failed.

Steps:
1. **Serialization of Success (`result_ok`)**:
   - The function takes `symlink::Success` which contains `file` (Option<Handle>), `attr` (Option<Attr>), and `wcc_data` (WccData).
   - It calls `option` to serialize `arg.file`. If present, it invokes `file_handle` to write the handle bytes.
   - It calls `option` to serialize `arg.attr`. If present, it invokes `file_attr` to write the attribute bytes.
   - It calls `wcc_data` to serialize `arg.wcc_data`, which writes the pre- and post-operation attributes of the directory.
2. **Serialization of Failure (`result_fail`)**:
   - The function takes `symlink::Fail` which contains `dir_wcc` (WccData).
   - It calls `wcc_data` to serialize `arg.dir_wcc`. Note that the error status code itself is typically serialized by the caller (the generic RPC layer), while this function handles the failure-specific data (WCC).

Edge Cases:
- **Optional Fields**: The `result_ok` function relies on `option` to handle cases where `file` or `attr` might be `None`. In XDR, this results in a `false` discriminant being written without subsequent data.
- **I/O Errors**: Any failure in the underlying `Write` implementation (e.g., buffer full) results in an immediate return of the `io::Error`.

Complexity:
- Time: O(N), where N is the total size of the serialized fields (handle, attributes, WCC data). The logic overhead is constant.
- Space: O(1) auxiliary space (streaming directly to the writer).

Determinism:
- Deterministic. Given the same input struct and a functioning `Write` implementation, the output byte sequence is identical.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer::files`**:
 - **`file_handle`**: Used to convert the `file::Handle` (a unique identifier for the new symlink) into the XDR opaque byte array format (`fhandle3`).
 - **`file_attr`**: Used to convert the `file::Attr` (metadata like mode, size, timestamps) into the XDR `fattr3` structure.
 - **`wcc_data`**: Used to serialize the `vfs::WccData` structure. This is critical for the NFS protocol to ensure the client can update its cache regarding the directory where the symlink was created. It handles the serialization of both pre-operation (`before`) and post-operation (`after`) attributes.

- **From `nfs_mamont::serializer`**:
 - **`option`**: This mechanism is used to serialize the `Option` wrappers for the file handle and attributes in `result_ok`. It ensures that the XDR "discriminated union" pattern is followed (writing a boolean to indicate presence/absence before the data).

- **From `nfs_mamont::vfs::symlink`**:
 - **`symlink::Success` and `symlink::Fail`**: These structs define the contract of what data must be serialized. The `Success` struct dictates that the response must contain a handle, attributes, and directory WCC data. The `Fail` struct dictates that only directory WCC data is required in the failure case.

---

## 4. Data Model

Entities:
- **`result_ok` Function**: A serializer mapping `symlink::Success` to `SYMLINK3resok`.
- **`result_fail` Function**: A serializer mapping `symlink::Fail` to `SYMLINK3resfail`.

Relations:
- **Mapping**: `result_ok` maps the fields of `symlink::Success` to the wire format in the order: `symlink_attributes` (handle), `symlink_attributes` (attributes), `dir_wcc` (WCC data).
- **Mapping**: `result_fail` maps the fields of `symlink::Fail` to the wire format in the order: `dir_wcc` (WCC data).

Global Invariants:
- The order of serialization in `result_ok` must strictly follow the NFSv3 specification for `SYMLINK3resok`: handle, attributes, then directory WCC data.
- The `result_fail` function assumes the error status code has been or will be serialized separately by the RPC dispatcher, as it only serializes the `dir_wcc` field associated with the failure.

## 5. Error Model

Error Types:
- **`std::io::Error`**: The only error type produced directly by this module.

Error Propagation Strategy:
- **Propagation**: Errors are propagated using the `?` operator from the underlying serialization functions (`option`, `file_handle`, `file_attr`, `wcc_data`). If any of these fail (e.g., due to a write error), the failure is immediately returned to the caller.

Recoverability:
- **Recoverable**: The caller (likely the RPC response handler) can catch the `io::Result` and handle the error (e.g., by closing the connection).

Panics:
- **Allowed**: No.
- **Conditions**: The code does not perform any operations that could panic (like unwrapping `None` or indexing out of bounds), relying entirely on the `Result` type for error handling.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines free-standing serialization functions.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **serialize the response payload for the NFSv3 `SYMLINK` procedure** into the XDR wire format. The system implements an NFSv3 server where the Virtual File System (VFS) layer handles the logic of creating symbolic links and returns high-level Rust structs (`symlink::Success` or `symlink::Fail`). These structs cannot be sent directly to the network because they do not conform to the XDR standard required by the NFS protocol.

A typical usage scenario of the system involves a client requesting the creation of a symbolic link. The VFS processes this request and returns a `symlink::Success` struct containing the new file's handle, its attributes, and the Weak Cache Consistency (WCC) data for the parent directory. The RPC layer then invokes the `result_ok` function from this module. This function orchestrates the serialization of these fields: it uses `option` to handle the optional handle and attributes, `file_handle` and `file_attr` to encode the specific file metadata, and `wcc_data` to encode the directory state changes. The resulting byte stream is then sent back to the client.

Inside the system, the following things happen and they use this module:
1.  **Protocol Compliance**: The module ensures that the specific field order and data types for the `SYMLINK3resok` and `SYMLINK3resfail` structures (as defined in RFC 1813) are strictly followed. This is essential for interoperability with standard NFS clients.
2.  **Cache Synchronization**: By serializing the `wcc_data` (in both success and failure cases), the module enables the client to update its cache for the directory where the link was created, preventing stale data views.
3.  **Abstraction Layer**: The module acts as an adapter between the generic VFS result types and the specific XDR serialization primitives. It allows the VFS to remain agnostic of the wire format details while ensuring the serializer correctly interprets the VFS data structures.

Without this module, the server would lack the specific logic to format the `SYMLINK` response, making it impossible to communicate the result of symbolic link creation operations to NFS clients.