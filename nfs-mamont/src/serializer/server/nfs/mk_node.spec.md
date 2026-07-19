<!-- SPEC_HASH: b51b20fcadbf2cefa05d2c244013aca98c66209e2234ce901544a1cc23c2dbe9 -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::mk_node
Rust File: src/serializer/server/nfs/mk_node.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**: Used to provide the `Write` trait, which defines the interface for the destination byte stream where the XDR data will be written, and `io::Result` for error handling during serialization.
- **`crate::serializer::files`**: Used to access high-level serialization functions for specific NFS data structures: `file_handle` (for the new file's handle), `file_attr` (for the new file's attributes), and `wcc_data` (for Weak Cache Consistency data of the directory).
- **`crate::serializer::option`**: Used to serialize optional fields (`Option<T>`) according to XDR rules (writing a boolean discriminant followed by the value if present). This is used for the file handle and attributes in the success response.
- **`crate::vfs::mk_node`**: Used as the source of the data structures `mk_node::Success` and `mk_node::Fail`, which represent the logical outcome of the VFS `MKNOD` operation that needs to be converted to the wire format.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide the specific XDR serialization logic for the `MKNOD3res` NFSv3 procedure response.
- To separate the serialization of the success payload (`MKNOD3resok`) from the failure payload (`MKNOD3resfail`), allowing the RPC layer to dispatch to the correct serializer based on the operation's result.

Inputs:
- `dest`: A mutable reference to a type implementing `std::io::Write` (e.g., a network buffer).
- `arg`: Either `mk_node::Success` (containing the new file handle, attributes, and directory WCC data) or `mk_node::Fail` (containing the directory WCC data).

Outputs:
- `io::Result<()>`: Indicates successful writing of the XDR structure to the destination or an I/O error.

Steps:
1. **Success Serialization (`result_ok`)**:
 - The function receives a `mk_node::Success` struct.
 - It serializes the `file` field (an `Option<file::Handle>`) using the `option` helper. If present, it delegates to `file_handle`.
 - It serializes the `attr` field (an `Option<file::Attr>`) using the `option` helper. If present, it delegates to `file_attr`.
 - It serializes the `wcc_data` field by delegating to the `wcc_data` function.
2. **Failure Serialization (`result_fail`)**:
 - The function receives a `mk_node::Fail` struct.
 - It serializes the `dir_wcc` field by delegating to the `wcc_data` function.
 - **Note**: The function does *not* serialize the error code (`arg.error`). It is assumed that the caller (the generic RPC layer) serializes the NFS status code (discriminant) before invoking this function to serialize the failure-specific body.

Edge Cases:
- **Missing Data**: If `arg.file` or `arg.attr` are `None` in the success case, the `option` helper correctly writes the XDR "false" discriminant, indicating the absence of data to the client.

Complexity:
- Time: O(N), where N is the total size of the handles, attributes, and WCC data being written. The logic itself is O(1).
- Space: O(1) auxiliary space (streaming directly to the writer).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer::files`**:
 - **`file_handle`**: Used to convert the internal `file::Handle` into the opaque XDR byte array required by the NFS protocol.
 - **`file_attr`**: Used to convert the internal `file::Attr` structure (mode, size, timestamps, etc.) into the `fattr3` XDR structure.
 - **`wcc_data`**: Used to serialize the `WccData` structure, which contains pre- and post-operation attributes for the directory. This is critical for the client to maintain cache consistency.

- **From `nfs_mamont::serializer`**:
 - **`option`**: Used to handle the XDR encoding of optional fields. In the NFSv3 `MKNOD3resok` structure, the file handle and attributes are technically unions (present or not), which map to Rust's `Option`. This helper ensures the correct boolean discriminant is written.

- **From `nfs_mamont::vfs::mk_node`**:
 - **`Success` and `Fail`**: These structs define the contract. The serializer relies on the specific field names and types (`Option<Handle>`, `Option<Attr>`, `WccData`) present in these structs to generate the correct wire format.

---

## 4. Data Model

Entities:
- **`result_ok` Function**: Maps `mk_node::Success` to XDR `MKNOD3resok`.
- **`result_fail` Function**: Maps `mk_node::Fail` to XDR `MKNOD3resfail`.

Relations:
- **Mapping**: `mk_node::Success.file` maps to the `obj` field in `MKNOD3resok`.
- **Mapping**: `mk_node::Success.attr` maps to the `obj_attributes` field in `MKNOD3resok`.
- **Mapping**: `mk_node::Success.wcc_data` and `mk_node::Fail.dir_wcc` map to the `dir_wcc` field in both `MKNOD3resok` and `MKNOD3resfail`.

Global Invariants:
- The output produced by `result_ok` must conform to the `MKNOD3resok` XDR definition in RFC 1813.
- The output produced by `result_fail` must conform to the `MKNOD3resfail` XDR definition in RFC 1813.

## 5. Error Model

Error Types:
- `std::io::Error`

Error Propagation Strategy:
- Errors are propagated using the `?` operator. If any underlying write operation (in `file_handle`, `file_attr`, or `wcc_data`) fails, the error is immediately returned to the caller.

Recoverability:
- Recoverable. The caller (likely the RPC response handler) can catch the error and abort the connection or return a server fault status.

Panics:
- Allowed: No
- Conditions: The code performs no explicit panics and relies on `Result` propagation.

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This module is used in order to **serialize the response payload for the NFSv3 `MKNOD` procedure**. The system contains an NFSv3 server that processes requests to create special file system nodes (like device files, named pipes, or sockets). When the Virtual File System (VFS) layer completes such a request, it returns a result structure (`Success` or `Fail`) containing Rust-native types. This module is responsible for converting those specific result structures into the binary XDR format defined by the NFSv3 protocol standard (RFC 1813) so they can be transmitted over the network.

A typical usage scenario of the system involves a client requesting the creation of a character device file. The VFS backend handles the creation and returns a `Success` struct containing the new file's handle and attributes. The RPC layer then calls `result_ok` from this module, passing the network buffer and the `Success` struct. The function serializes the handle, attributes, and directory cache data into the buffer. If the operation had failed (e.g., due to permissions), the RPC layer would call `result_fail`, which serializes the directory cache data (allowing the client to update its cache despite the error).

Inside the system, the following things happen and they use this module:
1. **Response Construction**: The RPC dispatcher uses this module to populate the body of the `MKNOD3res` XDR union. It relies on the separation of `result_ok` and `result_fail` to handle the different structures defined for success and failure cases in the protocol.
2. **Cache Consistency**: By delegating the serialization of `WccData` to the `wcc_data` helper (from `serializer::files`), this module ensures that the client receives the necessary pre- and post-operation attributes of the parent directory. This is vital for the client to validate its cache of the directory listing without performing a full `READDIR` operation again.
3. **Optional Field Handling**: The module uses the `option` serializer to correctly encode the file handle and attributes, which are optional in the protocol (e.g., if the server chooses not to return a handle for the new node). This ensures strict compliance with the XDR standard, preventing protocol violations that could confuse clients.