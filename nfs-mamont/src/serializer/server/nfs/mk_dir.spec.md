<!-- SPEC_HASH: 7a74a19a31e90044fe86bc570232541447eea4fd4c438aa95577238eb7e3468d -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::mk_dir
Rust File: src/serializer/server/nfs/mk_dir.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**: Used to provide the `Write` trait, which defines the interface for the destination byte stream (e.g., a network buffer), and `io::Result` for handling potential I/O errors during serialization.
- **`crate::serializer::files`**: Used to import specific serialization functions for file system entities: `file_handle` (for the directory handle), `file_attr` (for directory attributes), and `wcc_data` (for Weak Cache Consistency data). These functions handle the low-level XDR encoding for these specific types.
- **`crate::serializer`**: Used to import the `option` function. This is necessary to serialize optional fields (like the file handle and attributes in the success case) according to XDR union rules (writing a boolean discriminant followed by the value if present).
- **`crate::vfs::mk_dir`**: Used to import the `Success` and `Fail` structs. These represent the logical outcome of the `MKDIR` operation within the Virtual File System (VFS) layer and serve as the input data structures that this module converts into the wire format.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To serialize the result of the NFSv3 `MKDIR` procedure into the XDR (External Data Representation) format.
- To provide separate serialization paths for successful (`MKDIR3resok`) and failed (`MKDIR3resfail`) responses, as defined by the NFSv3 protocol (RFC 1813).

Inputs:
- `dest`: A mutable reference to a type implementing `std::io::Write`, representing the buffer or stream where the XDR bytes will be written.
- `arg`: Either `mk_dir::Success` (containing the new directory's handle, attributes, and WCC data) or `mk_dir::Fail` (containing the parent directory's WCC data).

Outputs:
- `io::Result<()>`: Indicates successful serialization of the entire structure into the destination or an error if the write operation fails.

Steps:
1. **Success Serialization (`result_ok`)**:
   - The function first serializes the `file` field (the handle of the created directory). It uses the `option` serializer, passing `file_handle` as the closure to handle the actual bytes if the handle is present.
   - Next, it serializes the `attr` field (the attributes of the created directory). Similarly, it uses the `option` serializer with `file_attr` as the closure.
   - Finally, it serializes the `wcc_data` field (Weak Cache Consistency data for the parent directory) by calling `wcc_data`.
2. **Failure Serialization (`result_fail`)**:
   - The function serializes the `dir_wcc` field (Weak Cache Consistency data for the parent directory) by calling `wcc_data`. This is the only data present in a failure response for `MKDIR`.

Edge Cases:
- **Optional Fields**: The `Success` struct contains `Option` types for `file` and `attr`. The `option` serializer correctly handles the case where these are `None` by writing a `false` discriminant and omitting the data.
- **Write Failures**: Any error returned by the underlying `Write` implementation or the helper serializers (`file_handle`, `file_attr`, `wcc_data`) is propagated immediately via the `?` operator, aborting the serialization process.

Complexity:
- Time: O(1) in terms of control flow logic, but effectively O(N) where N is the size of the data being written (delegated to the helper serializers).
- Space: O(1) auxiliary space (streaming directly to the writer).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::serializer::files`**:
  - **`file_handle`**: Used to serialize the opaque file handle of the newly created directory. It ensures the handle is written as a variable-length opaque array with correct length prefix and padding.
  - **`file_attr`**: Used to serialize the attributes (mode, size, timestamps, etc.) of the newly created directory. It maps the internal `file::Attr` struct to the XDR `fattr3` structure.
  - **`wcc_data`**: Used to serialize the `WccData` struct. This is critical for both success and failure cases to provide the client with pre- and post-operation attributes of the parent directory, enabling cache consistency checks.

- **From `crate::serializer`**:
  - **`option`**: Used to serialize the `Option<file::Handle>` and `Option<file::Attr>` fields in the `Success` struct. It implements the XDR union logic where a boolean indicates presence, followed by the value if true.

- **From `crate::vfs::mk_dir`**:
  - **`Success`**: The input struct containing the data to be serialized on success. It aggregates the handle, attributes, and WCC data.
  - **`Fail`**: The input struct containing the data to be serialized on failure. It aggregates the WCC data for the parent directory.

---

## 4. Data Model

Entities:
- **XDR Response Messages**: The implicit binary layouts defined by RFC 1813 for `MKDIR3resok` and `MKDIR3resfail`.
  - `MKDIR3resok`: Contains an optional file handle, optional attributes, and WCC data.
  - `MKDIR3resfail`: Contains WCC data.

Relations:
- **Mapping**: `result_ok` maps `vfs::mk_dir::Success` to `MKDIR3resok`.
- **Mapping**: `result_fail` maps `vfs::mk_dir::Fail` to `MKDIR3resfail`.

Global Invariants:
- The output byte stream must conform to the XDR standard (Big Endian, 4-byte aligned) as enforced by the helper serializers.

## 5. Error Model

Error Types:
- `std::io::Error`

Error Propagation Strategy:
- Propagation via the `?` operator. Errors from the underlying `Write` trait or helper serialization functions are returned directly to the caller.

Recoverability:
- Recoverable. The caller (likely the RPC layer) can catch the `io::Result::Err` and handle the connection failure (e.g., by closing the socket).

Panics:
- Allowed: No
- Conditions: The code does not perform any operations that could panic (e.g., no indexing or unwrapping) other than those potentially triggered by the underlying `Write` implementation.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines free-standing serialization functions.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **serialize the response for the NFSv3 `MKDIR` procedure** into the XDR wire format required for network transmission. The system consists of an NFSv3 server that processes file system requests. When a client requests to create a directory, the VFS (Virtual File System) layer executes the operation and returns a result structure (`mk_dir::Success` or `mk_dir::Fail`). This result is a high-level Rust object that cannot be sent directly over the network.

A typical usage scenario of the system involves the server receiving an `MKDIR` RPC call. The server logic invokes the VFS to create the directory. Upon completion, the system must send a response packet back to the client. This module is invoked to populate that packet. It takes the VFS result and, using the `serializer::files` and `serializer` dependencies, converts the fields (file handle, attributes, WCC data) into a byte stream compliant with RFC 1813.

Inside the system, the following things happen and they use this module:
1. **Response Construction**: The RPC layer calls `result_ok` if the directory creation succeeded. This function writes the new directory's handle and attributes (if available) and the parent directory's WCC data to the output buffer.
2. **Error Reporting**: If the operation failed, the RPC layer calls `result_fail`. This function writes only the parent directory's WCC data to the buffer, allowing the client to update its cache despite the error.
3. **Protocol Compliance**: By delegating to `file_handle`, `file_attr`, and `wcc_data`, this module ensures that the specific XDR encoding rules (like padding and byte order) for these complex types are correctly applied without duplicating logic.

Without this module, the server would lack the specific logic to format the `MKDIR` response, leaving a gap between the internal VFS result and the external protocol expected by NFS clients. It acts as the final adapter in the request processing pipeline for this specific operation.