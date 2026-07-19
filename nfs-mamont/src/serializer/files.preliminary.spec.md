<!-- SPEC_HASH: b466694e94fa62bca4146dc839cb184f9cb52e391197d819d228d44664fb4980 -->
# Module Specification

Module: nfs_mamont::serializer::files
Rust File: src/serializer/files.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **std::io**: Used to provide the `Write` trait, which defines the destination sink for the serialized XDR bytes, and `io::Error`/`ErrorKind` for handling I/O failures or invalid data conversions (e.g., non-UTF-8 paths).
- **crate::serializer**: Used to access low-level XDR serialization primitives such as `u32`, `u64`, `array`, `option`, `string_max_size`, `variant`, and `usize_as_u32`. These functions handle the specific byte ordering (Big Endian) and padding rules required by the XDR standard.
- **crate::vfs::file**: Used as the source of domain-specific data types (`Time`, `Handle`, `Attr`, `Name`, `Path`, `Type`, `WccAttr`) that need to be converted into the NFSv3 wire format.
- **crate::vfs**: Used to import composite VFS types (`DirOpArgs`, `WccData`, `Error`) and constants (`MAX_PATH_LEN`) that define the structure of NFS operation arguments and results.
- **crate::consts::nfsv3**: Used to import `NFS3_FHSIZE`, which defines the maximum size of a file handle in the NFSv3 protocol, required for serializing the length prefix of file handles.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To transform high-level Virtual File System (VFS) data structures into their binary representation according to the NFSv3 XDR (External Data Representation) standard.
- To act as a serialization adapter that maps semantic Rust types (e.g., `file::Attr`) to protocol-specific wire formats (e.g., `fattr3`), ensuring that length limits, padding, and byte ordering are correctly applied.

Inputs:
- A mutable reference to a destination implementing `std::io::Write` (e.g., a network buffer or file).
- A VFS data structure instance (e.g., `file::Attr`, `file::Handle`, `vfs::WccData`).

Outputs:
- `io::Result<()>`: Indicates successful serialization of the structure into the destination or an error if writing fails or data is invalid.

Steps:
1. **Primitive Serialization**: Functions like `nfs_time` and `file_type` extract fields from the input structs and pass them to the underlying `serializer` primitives (e.g., `u32`, `variant`).
2. **Composite Serialization**: Functions like `file_attr` serialize complex structures by sequentially invoking serializers for each field in the order defined by the NFSv3 specification (type, mode, nlink, uid, gid, size, used, device, fs_id, file_id, atime, mtime, ctime).
3. **String Serialization**: `file_name` and `file_path` serialize strings using `string_max_size`. `file_path` additionally converts the underlying `PathBuf` to a UTF-8 `String`, returning an error if the path is not valid UTF-8.
4. **Optional Serialization**: `wcc_data` serializes `WccData` by treating the `before` and `after` fields as optional XDR unions. It uses the `option` serializer to write a boolean discriminant followed by the attribute data if present.
5. **Handle Serialization**: `file_handle` writes the file handle as a variable-length opaque array. It first writes the length (fixed to `NFS3_FHSIZE`) using `usize_as_u32`, followed by the raw bytes of the handle using `array`.

Edge Cases:
- **Path Conversion**: The `file_path` function attempts to convert the internal `PathBuf` to a `String`. If the path contains non-UTF-8 characters, the function returns an `io::Error` with `ErrorKind::InvalidInput`.
- **String Padding**: The `string_max_size` function (from the dependency) is responsible for adding null bytes to pad the string length to a multiple of 4 bytes, as required by XDR. This behavior is verified in the module's tests (e.g., `test_file_path_with_padding`).

Complexity:
- Time: O(N), where N is the size of the data structure being serialized (linear in the number of fields/bytes).
- Space: O(1) auxiliary space (streaming directly to the writer), excluding the buffer managed by the `Write` implementation.

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `crate::serializer`**: The `u32`, `u64`, `array`, `string_max_size`, `option`, and `variant` functions are essential. They abstract away the details of XDR encoding, such as big-endian byte ordering and 4-byte alignment padding. The `string_max_size` function specifically enforces the maximum length constraints defined in the NFSv3 protocol.
- **From `crate::vfs::file`**: The `Handle`, `Name`, `Path`, `Attr`, `Type`, `Time`, and `WccAttr` structs provide the validated data. The `Name` and `Path` types guarantee that the strings do not contain invalid characters (like slashes in names) or exceed system limits, though the serializer re-checks length against protocol limits.
- **From `crate::vfs`**: The `WccData` struct provides the container for pre- and post-operation attributes, which is serialized as a structure containing two optional unions. The `Error` enum provides the discriminants used for NFS status codes.

---

## 4. Data Model

Entities:
- **Serializers**: Stateless functions (`nfs_time`, `file_attr`, `wcc_data`, etc.) that act as converters.
- **XDR Structures**: The implicit binary layouts defined by RFC 1813 (e.g., `fattr3`, `nfstime3`, `wcc_data`) that the output bytes conform to.

Relations:
- **Composition**: `file_attr` calls `file_type` and `nfs_time`.
- **Composition**: `wcc_data` calls `wcc_attr` and `file_attr`.
- **Mapping**: `vfs::file::Attr` maps to XDR `fattr3`. `vfs::file::Time` maps to XDR `nfstime3`. `vfs::WccData` maps to XDR `wcc_data`.

Global Invariants:
- The output byte stream produced by these functions must conform to the XDR encoding standard (Big Endian, 4-byte aligned) as specified in RFC 1813.
- String lengths written to the wire must not exceed the limits defined in `vfs` (`MAX_NAME_LEN`, `MAX_PATH_LEN`).

## 5. Error Model

Error Types:
- `std::io::Error`

Error Propagation Strategy:
- Errors are propagated using the `?` operator. This includes I/O errors from the underlying `Write` implementation and conversion errors (e.g., `InvalidInput` when a path cannot be converted to UTF-8).

Recoverability:
- Recoverable. The functions return `Result`, allowing the caller to handle serialization failures (e.g., by dropping the connection or sending an NFS error).

Panics:
- Allowed: No
- Conditions: The code does not explicitly panic. It relies on the `Write` trait and `Result` handling.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines free-standing functions and does not implement traits for its own types.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to serialize the internal state of the Virtual File System (VFS) into the standardized NFSv3 wire format required for network communication. The system contains an NFSv3 server that processes file system requests (like `LOOKUP`, `READ`, or `GETATTR`). When the server completes a request, the VFS layer produces high-level Rust structures representing the result (e.g., file attributes, directory entries, or error statuses). These internal structures cannot be sent directly to the client because they use Rust-specific memory layouts and types.

A typical usage scenario of the system involves the server handling a `GETATTR` request. The VFS retrieves file metadata and returns a `file::Attr` struct. The system then invokes the `file_attr` function from this module, passing the network buffer and the `Attr` struct. The function translates the fields (mode, uid, size, timestamps, etc.) into a sequence of bytes conforming to the `fattr3` XDR definition defined in RFC 1813. Inside the system, this translation is critical for interoperability; it ensures that a Linux NFS client can correctly interpret the permissions, file sizes, and modification times reported by the server. Without this module, the server would lack the logic to bridge the gap between its internal object-oriented representation of files and the flat, binary protocol expected by NFS clients, making communication impossible.