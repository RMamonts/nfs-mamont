<!-- SPEC_HASH: f35e1d5af0fbe38101486576a279fb2b13f9c711cd14691821dd026b7782b7f5 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::lookup
Rust File: src/serializer/server/nfs/lookup.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io` and `std::io::Write`**: Used to define the destination sink for the XDR bytes. The `Write` trait allows the serializer to write into any buffer or stream (e.g., a network socket), and `io::Result` is used to propagate write errors.
- **`crate::serializer::files`**: Used to import `file_handle` and `file_attr`. These functions encapsulate the logic for serializing the specific binary formats of file handles and file attributes as defined by the NFSv3 protocol.
- **`crate::serializer::option`**: Used to import the `option` serializer. This function handles the XDR encoding for optional fields (unions), writing a boolean discriminant followed by the value if present.
- **`crate::vfs::lookup`**: Used to import the `Success` and `Fail` structs. These represent the high-level result of a VFS lookup operation, which serve as the source data that this module converts into the wire format.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To convert the result of a Virtual File System (VFS) `LOOKUP` operation into the binary XDR format defined by the NFSv3 protocol (RFC 1813).
- To handle the serialization of both successful and failed lookup responses, ensuring that Weak Cache Consistency (WCC) data (directory attributes) is correctly included in both cases.

Inputs:
- `dest`: A mutable reference to a type implementing `std::io::Write` (the output buffer).
- `arg`: Either a `lookup::Success` struct (for successful lookups) or a `lookup::Fail` struct (for failed lookups).

Outputs:
- `io::Result<()>`: Indicates successful serialization of the entire structure into the destination or an I/O error if writing fails.

Steps:
1. **Serialization of Success (`result_ok`)**:
   - The function receives a `lookup::Success` struct containing the file handle, optional file attributes, and optional directory attributes.
   - It invokes `file_handle` to serialize the `file` field (the handle of the found object).
   - It invokes `option` to serialize the `file_attr` field. If present, the closure calls `file_attr` to write the attributes.
   - It invokes `option` to serialize the `dir_attr` field. If present, the closure calls `file_attr` to write the directory attributes.
2. **Serialization of Failure (`result_fail`)**:
   - The function receives a `lookup::Fail` struct containing the error code and optional directory attributes.
   - It invokes `option` to serialize the `dir_attr` field. If present, the closure calls `file_attr` to write the directory attributes.
   - Note: The error code itself is not serialized in this specific module; it is assumed to be handled by the caller or a higher-level serializer (e.g., the RPC status header).

Edge Cases:
- **Missing Attributes**: The `file_attr` and `dir_attr` fields in `Success`, and `dir_attr` in `Fail`, are optional. The `option` serializer correctly handles the case where these are `None` by writing a `false` discriminant and omitting the data.
- **Write Failures**: If the underlying `Write` stream returns an error (e.g., buffer full), the error is propagated immediately via the `?` operator, halting serialization.

Complexity:
- Time: O(N), where N is the total size of the file handle and attributes being written. The logic is a linear sequence of writes.
- Space: O(1) auxiliary space (excluding the buffer managed by the `Write` implementation).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer::files`**:
  - **`file_handle`**: Used to write the variable-length opaque file handle. This ensures the handle is prefixed with its length and padded correctly to 4-byte boundaries.
  - **`file_attr`**: Used to write the full set of file attributes (type, mode, size, timestamps, etc.) conforming to the `fattr3` XDR structure.
- **From `nfs_mamont::serializer` (via `crate::serializer::option`)**:
  - **`option`**: Used to serialize optional fields. It writes a boolean (1 for present, 0 for absent) and, if present, executes the provided closure to serialize the inner value. This is critical for the WCC (Weak Cache Consistency) attributes which may or may not be returned by the VFS.
- **From `nfs_mamont::vfs::lookup`**:
  - **`Success` and `Fail`**: These structs define the data contract. `Success` provides the result of the lookup (handle + attributes), while `Fail` provides the context for the failure (directory attributes for cache validation).

---

## 4. Data Model

Entities:
- **Serializers**: Stateless functions `result_ok` and `result_fail` that act as adapters between VFS types and the XDR wire format.
- **XDR Structures**: The implicit binary layouts defined by RFC 1813 for `LOOKUP3resok` (success) and `LOOKUP3resfail` (failure).

Relations:
- **Mapping**: `result_ok` maps `vfs::lookup::Success` to `LOOKUP3resok`.
- **Mapping**: `result_fail` maps `vfs::lookup::Fail` to `LOOKUP3resfail`.
- **Composition**: Both functions compose `file_handle`, `file_attr`, and `option` serializers to build the complete response.

Global Invariants:
- The order of fields in the output stream must strictly follow the NFSv3 specification:
  - For `result_ok`: `file` (handle) -> `obj_attributes` (optional) -> `dir_attributes` (optional).
  - For `result_fail`: `dir_attributes` (optional).

## 5. Error Model

Error Types:
- `std::io::Error`

Error Propagation Strategy:
- Errors are propagated using the `?` operator. If any underlying serialization function (`file_handle`, `file_attr`, `option`) returns an `Err`, it is immediately returned to the caller.

Recoverability:
- Recoverable. The caller (typically the RPC response handler) can catch the `io::Result` and handle the error (e.g., by closing the connection or logging).

Panics:
- Allowed: No
- Conditions: The code relies on the `Write` trait and `Result` handling; there are no explicit `panic!` calls or `unwrap()` calls.

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

This module is used in order to **serialize the response payload for the NFSv3 `LOOKUP` procedure** into the network protocol format. The system contains an NFSv3 server that processes file system requests. When a client requests to look up a file name in a directory, the VFS (Virtual File System) layer performs the operation and returns a result containing file handles and attributes. These internal Rust structures cannot be sent directly over the network.

A typical usage scenario of the system involves the server receiving a `LOOKUP` request. The VFS backend locates the file and returns a `vfs::lookup::Success` struct containing the file's handle and metadata. The RPC layer then invokes the `result_ok` function from this module. This function translates the high-level `Success` struct into the specific byte sequence defined by the NFSv3 XDR specification (`LOOKUP3resok`). This includes writing the file handle, the attributes of the found file (if available), and the post-operation attributes of the directory (for cache consistency).

Inside the system, the following things happen and they use this module:
1. **Protocol Compliance**: The module ensures that the response adheres to the strict byte-ordering and padding rules of XDR. It delegates the low-level byte writing to `serializer::files` and `serializer::option`, but it orchestrates the specific order required for a `LOOKUP` response.
2. **Cache Consistency**: By serializing the `dir_attr` (directory attributes) in both `result_ok` and `result_fail`, the module enables the client to perform Weak Cache Consistency (WCC) checks. This allows the client to validate if the directory changed during the operation, which is vital for distributed file system consistency.
3. **Error Handling**: In the case of a failure (e.g., file not found), the `result_fail` function is used. It ensures that even though the lookup failed, the directory attributes are still sent if available, allowing the client to update its cache for the directory itself.

Without this module, the server would lack the specific logic to format `LOOKUP` responses, making it impossible for clients to navigate the file system hierarchy or maintain consistent caches.