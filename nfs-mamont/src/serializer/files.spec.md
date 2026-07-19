<!-- SPEC_HASH: b466694e94fa62bca4146dc839cb184f9cb52e391197d819d228d44664fb4980 -->
# Module Specification

Module: nfs_mamont::serializer::files
Rust File: src/serializer/files.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**: Used to provide the `Write` trait, which defines the destination for the serialized bytes, and `ErrorKind` (specifically `InvalidInput`), which is used to signal conversion failures (e.g., non-UTF-8 paths) or constraint violations.
- **`crate::consts::nfsv3`**: Used to import `NFS3_FHSIZE`, which defines the fixed size of the file handle in the NFSv3 protocol. This constant is used to determine the length prefix when serializing file handles.
- **`crate::serializer`**: Used to import low-level XDR serialization primitives such as `u32`, `u64`, `array`, `option`, `string_max_size`, `variant`, and `usize_as_u32`. These functions handle the specifics of byte ordering (Big Endian), padding, and basic type encoding, allowing this module to focus on mapping domain types to the protocol structure.
- **`crate::vfs`**: Used to import the `Error` enum (for status codes), `WccData` (for weak cache consistency data), `DirOpArgs` (for directory operation arguments), and the constants `MAX_NAME_LEN` and `MAX_PATH_LEN` (for validating string lengths).
- **`crate::vfs::file`**: Used to import the domain-specific types that represent file system entities: `Time`, `Handle`, `Type`, `Attr`, `WccAttr`, `Name`, and `Path`. These types are the inputs that are transformed into the XDR format.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a set of serialization functions that map the internal Virtual File System (VFS) data structures to their corresponding NFSv3 XDR (External Data Representation) wire formats.
- To ensure that the translation from Rust types to protocol bytes strictly adheres to the field order, data types, and size constraints defined in RFC 1813.

Inputs:
- `dest`: A mutable reference to a type implementing the `std::io::Write` trait, serving as the byte sink.
- Domain objects: Instances of types defined in `vfs::file` (e.g., `file::Attr`, `file::Handle`) or `vfs` (e.g., `vfs::WccData`).

Outputs:
- `io::Result<()>`: Indicates success or failure of the write operation.

Steps:
1. **Primitive Mapping**: Functions like `nfs_time` and `file_type` decompose complex types into primitives (e.g., `u32`) and serialize them sequentially using the primitives from the `serializer` module.
2. **Struct Serialization**: Functions like `file_attr` and `wcc_attr` serialize structures by invoking the appropriate serializer for each field in the specific order required by the NFSv3 protocol (e.g., type followed by mode, followed by nlink, etc.).
3. **Optional Field Handling**: The `wcc_data` function serializes the `before` and `after` attributes using the `option` primitive. This involves writing a boolean discriminator (present/absent) followed by the attribute data if present.
4. **String Serialization**: The `file_name` and `file_path` functions serialize strings as variable-length opaque data with a 4-byte length prefix. They enforce maximum length constraints (`MAX_NAME_LEN`, `MAX_PATH_LEN`) via the `string_max_size` helper.
5. **Path Conversion**: The `file_path` function converts an internal `PathBuf` to a UTF-8 string. If the path contains non-UTF-8 characters, it returns an `io::Error` with `ErrorKind::InvalidInput`.
6. **Fixed Array Serialization**: The `file_handle` function serializes the file handle as a fixed-length opaque array. It writes the size (`NFS3_FHSIZE`) as a `u32` followed by the raw bytes of the handle.

Edge Cases:
- **Invalid Path Encoding**: `file_path` will return an error if the underlying `PathBuf` cannot be converted into a UTF-8 string, as XDR strings are defined as UTF-8 (or ASCII) encoded.
- **Length Constraints**: While the `vfs::file` module guarantees that `Name` and `Path` are constructed within length limits, this module enforces those limits again during serialization via `string_max_size` to ensure protocol compliance.

Complexity:
- Time: O(N), where N is the total number of bytes written to the destination. For fixed-size structures like `Attr` or `Handle`, this is effectively O(1).
- Space: O(1) auxiliary space (excluding the buffer managed by the `Write` implementation).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer`**:
 - **XDR Primitives**: This module relies entirely on the `serializer` module to perform the actual byte manipulation. It uses `u32` and `u64` for integers, `array` for fixed byte sequences, `option` for optional fields, `string_max_size` for bounded strings, and `variant` for encoding enum discriminants. It assumes these primitives handle Big Endian encoding and 4-byte padding correctly.
 - **Error Propagation**: It relies on the `serializer` module to return `std::io::Result` for any I/O or validation failures, which it then propagates to the caller.

- **From `nfs_mamont::vfs::file`**:
 - **Type Safety**: This module consumes the strongly-typed wrappers defined in `vfs::file` (e.g., `Handle`, `Name`). It assumes that the invariants of these types (e.g., `Name` does not contain slashes) hold true, allowing the serializer to focus on encoding rather than validation.
 - **Enum Mapping**: It uses the `Type` enum, which derives `ToPrimitive`, to map file types (Regular, Directory, etc.) to their integer values defined in the NFSv3 spec.

- **From `nfs_mamont::vfs`**:
 - **Composite Structures**: It serializes `WccData` and `DirOpArgs`, which are composite types defined in the `vfs` module. These structures aggregate the primitive types from `vfs::file` into the specific argument/result structures used by NFS procedures.
 - **Constants**: It uses `MAX_NAME_LEN` and `MAX_PATH_LEN` to ensure that serialized strings do not exceed the limits allowed by the protocol or the server configuration.

---

## 4. Data Model

Entities:
- **Serializers**: Stateless functions that act as adapters between VFS types and the XDR byte stream.
- **XDR Structures**: The implicit binary structures defined by the NFSv3 RFC (e.g., `fattr3`, `nfs_fh3`, `wcc_data`) which are the output of these functions.

Relations:
- **Mapping**: Each public function in this module maps a specific VFS type to a specific XDR structure (e.g., `file_attr` maps `file::Attr` to `fattr3`).
- **Composition**: Complex serializers like `file_attr` and `wcc_data` are composed of calls to simpler serializers (e.g., `nfs_time`, `file_type`).

Global Invariants:
- **Alignment**: All serialized data is implicitly padded to 4-byte boundaries because the underlying `serializer` primitives enforce this.
- **Fixed Sizes**: The file handle is always serialized with a length of `NFS3_FHSIZE`.

## 5. Error Model

Error Types:
- **`std::io::Error`**: The only error type returned by these functions.

Error Propagation Strategy:
- **Direct Propagation**: Errors returned by the underlying `Write` implementation or the `serializer` primitives are propagated immediately using the `?` operator.
- **Conversion Errors**: The `file_path` function explicitly creates an `io::Error` with `ErrorKind::InvalidInput` if the path cannot be converted to a UTF-8 string.

Recoverability:
- **Recoverable**: All functions return `Result`, allowing the caller to handle the error (e.g., by aborting the RPC response and sending an NFS status code).

Panics:
- **Allowed**: No.
- **Conditions**: The code avoids panics by using checked conversions and returning `io::Result` for all failure modes.

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This module is used in order to **translate the server's internal file system representation into the NFSv3 wire format**. The system contains a complex architecture where the storage backend (VFS) operates on safe, high-level Rust types, but the network layer must transmit raw bytes conforming to the XDR standard defined in RFC 1813. This module serves as the critical adapter between these two layers.

A typical usage scenario of the system involves the server processing a `GETATTR` request. The VFS layer retrieves the file metadata and returns a `file::Attr` struct. The RPC layer then invokes the `file_attr` function from this module, passing the network buffer and the `Attr` struct. The function serializes the fields (type, mode, size, timestamps, etc.) into the buffer in the exact order and format expected by the NFS client. Without this module, the server would have no standardized way to convert its internal state into a protocol-compliant response.

Inside the system, the following things happen and they use this module:
1. **Response Encoding**: The RPC layer uses functions like `file_attr`, `wcc_data`, and `file_handle` to encode the success results of NFS procedures.
2. **Error Encoding**: The `error` function is used to convert internal `vfs::Error` codes into the integer status codes required by the NFS protocol.
3. **Argument Encoding**: While primarily used for responses, functions like `dir_op_arg` can be used to serialize arguments for procedures like `REMOVE` or `RENAME` if the server needs to log or forward requests.

This module ensures that the binary representation of the server's data is deterministic and interoperable with standard NFS clients, abstracting away the tedious details of byte ordering, padding, and field sequencing from the higher-level logic.