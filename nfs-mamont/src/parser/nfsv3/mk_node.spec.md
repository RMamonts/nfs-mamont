<!-- SPEC_HASH: dc75e207d9e80deb0a2df7a66a8003fd797a0387cff5a48d70138ffcf7a2bbb0 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::mk_node
Rust File: src/parser/nfsv3/mk_node.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the generic source trait for the `src` parameter in parsing functions. It allows the parser to consume bytes from network streams or test buffers.
- **`crate::parser::nfsv3::create`**: Used to import the `new_attr` function. This function is called to parse the `sattr3` (set attributes) structure, which defines the initial attributes (mode, uid, gid, size, timestamps) for the new node, regardless of the node type.
- **`crate::parser::nfsv3::file`**: Used to import `handle` and `file_name` functions. These are used in the `args` function to parse the directory file handle and the name of the new node, which constitute the `DirOpArgs` part of the request.
- **`crate::parser::primitive`**: Used to import the `u32` function. This is used to read the discriminant determining the type of node to create (Block, Char, Socket, Fifo) and to read the major/minor numbers for device files.
- **`crate::parser`**: Used to import the `Error` enum and `Result` type alias. This allows the module to report parsing failures (e.g., `EnumDiscMismatch`, `IO`) consistently with the rest of the parser subsystem.
- **`crate::vfs::file`**: Used to import the `Device` struct. This is used to wrap the major and minor numbers read from the stream for character and block device creation.
- **`crate::vfs::mk_node`**: Used to import the `Args` and `What` types. These are the target data structures that the parsing functions populate, representing the complete arguments for the VFS `MKNOD` operation.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `MKNOD` procedure from a byte stream into the high-level `vfs::mk_node::Args` structure.
- To handle the conditional parsing logic required by the `ftype3` union (mapped to `vfs::mk_node::What`), which switches between parsing device numbers (for Block/Char) or just attributes (for Socket/Fifo) based on a discriminant.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream (e.g., network socket or test cursor) from which XDR-encoded data is read.

Outputs:
- `Result<mk_node::Args>`: The fully parsed arguments for the MKNOD procedure.
- `Result<mk_node::What>`: The parsed node type enum variant containing the specific data for that type.

Steps:
1. **`args` function**:
 - Calls `file::handle(src)` to read the directory file handle where the node will be created.
 - Calls `file::file_name(src)` to read the name of the new node.
 - Constructs `vfs::DirOpArgs` from the handle and name.
 - Calls `what(src)` to parse the type-specific data.
 - Constructs and returns `mk_node::Args` containing the directory operation arguments and the `what` data.
2. **`what` function**:
 - Reads a `u32` discriminant from the stream.
 - Matches the discriminant against known NFSv3 file type constants:
  - `3` (Block Device): Calls `new_attr(src)` to parse attributes, reads `major` (u32), reads `minor` (u32), constructs `Device`, and returns `What::Block`.
  - `4` (Character Device): Calls `new_attr(src)` to parse attributes, reads `major` (u32), reads `minor` (u32), constructs `Device`, and returns `What::Char`.
  - `6` (Socket): Calls `new_attr(src)` to parse attributes and returns `What::Socket`.
  - `7` (Fifo): Calls `new_attr(src)` to parse attributes and returns `What::Fifo`.
 - If the discriminant does not match 3, 4, 6, or 7, returns `Error::EnumDiscMismatch`.

Edge Cases:
- **Invalid Discriminants**: If the type integer is not 3, 4, 6, or 7, the parser returns `Error::EnumDiscMismatch`.
- **Stream Exhaustion**: If the stream ends prematurely while reading attributes or device numbers, the underlying `primitive` or `create` parsers will return an `Error::IO`.

Complexity:
- Time: O(1) for fixed-size fields (handles, integers, discriminants). O(N) for variable-length fields (filenames, optional attributes), where N is the length of the data.
- Space: O(1) for stack-allocated structures. O(N) for heap-allocated strings within `file::Name` and optional attributes.

Determinism:
- Deterministic (Given the same input byte stream, the functions produce the same output or error).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::create`**:
 - **`new_attr`**: This mechanism is critical for parsing the `sattr3` structure. In the `MKNOD` procedure, the client can specify initial attributes for the new node. `new_attr` handles the parsing of optional fields (mode, uid, gid, size, atime, mtime), which is reused for all four types of nodes supported by this parser.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle` and `file_name`**: These functions are used to establish the context of the operation (the parent directory). `handle` ensures the file identifier is exactly 64 bytes, and `file_name` enforces length limits and UTF-8 validity.

- **From `nfs_mamont::parser::primitive`**:
 - **`u32`**: Used to read the discriminant that determines the node type and the major/minor numbers for device files. This handles the Big-Endian conversion required by the XDR standard.

- **From `nfs_mamont::vfs::mk_node`**:
 - **`What` Enum**: The parsing logic in the `what` function is strictly driven by the variants of this enum (`Block`, `Char`, `Socket`, `Fifo`). The discriminant read from the wire determines which variant to construct and dictates whether additional data (device numbers) needs to be parsed.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a transformer, converting byte streams into the entities defined in `nfs_mamont::vfs::mk_node`.

Relations:
- **Transformation**: The functions in this module act as adapters between the `Read` stream and the `vfs::mk_node` structs.
- **Composition**: `mk_node::Args` contains `vfs::DirOpArgs` (parsed via `file` functions) and `mk_node::What` (parsed via `what`). `mk_node::What` contains `set_attr::NewAttr` (parsed via `new_attr`) and optionally `vfs::file::Device`.

Global Invariants:
- **Discriminant Mapping**: The integer values 3, 4, 6, and 7 are strictly mapped to `Block`, `Char`, `Socket`, and `Fifo` respectively, corresponding to the `ftype3` enum in the NFSv3 specification.
- **Attribute Presence**: The `new_attr` structure is parsed for all valid node types, meaning attributes are always present in the wire format for `MKNOD`.

## 5. Error Model

Error Types:
- **`parser::Error`**: The primary error type used throughout the module.
 - `Error::EnumDiscMismatch`: Returned when the node type discriminant is not 3, 4, 6, or 7.
 - `Error::IO`: Propagated from the underlying `Read` stream or from dependency parsers if the stream ends unexpectedly.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used extensively to propagate errors from `primitive` parsers, `file` parsers, and `create` parsers.
- **Direct Return**: Logic errors (invalid discriminants) are returned immediately as `Error::EnumDiscMismatch`.

Recoverability:
- **Unrecoverable for the current operation**: If parsing fails, the stream cursor is likely in an undefined state relative to the higher-level RPC message. The caller (typically the RPC dispatcher) must discard the current request.

Panics:
- Allowed: No
- Conditions: The code uses `match` with a catch-all `_` branch returning errors, and relies on dependency parsers which handle I/O errors via `Result`. No `unwrap` or `expect` calls are present.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **deserialize the arguments for the NFSv3 MKNOD procedure** from the network wire format into the structured types required by the server's Virtual File System (VFS). The system contains a parser hierarchy where `primitive` handles raw bytes, `file` handles identifiers, and `create` handles attributes. This module sits in the middle, implementing the specific logic required to interpret the `MKNOD3args` XDR structure, which is unique because it involves a union type (`What`) that varies significantly based on the type of file system node being created (device vs. socket vs. fifo).

A typical usage scenario of the system involves the RPC dispatcher receiving a `MKNOD` request from an NFS client. The dispatcher identifies the procedure number and invokes the `args` function in this module. The function reads the directory handle and filename to locate where the node should be created. It then reads a type discriminant. If the type is a device (Block or Char), it parses the initial attributes and the device major/minor numbers. If the type is a Socket or Fifo, it parses only the initial attributes. The resulting `mk_node::Args` struct is then passed to the VFS implementation to perform the actual node creation.

Inside the system, the following things happen and they use this module:
- **Type-Specific Deserialization**: The `what` function implements the logic for the `ftype3` union. It distinguishes between creating device nodes (which require numeric IDs) and IPC nodes (sockets/fifos, which do not). This is critical for supporting the full range of special file creation allowed by the NFSv3 protocol.
- **Attribute Reuse**: By delegating attribute parsing to the `new_attr` function from the `create` module, this module ensures that the logic for interpreting `sattr3` (optional mode, uid, gid, etc.) is consistent across file creation, directory creation, and special node creation.
- **Contextual Parsing**: The `args` function combines the generic directory operation context (from `file` module) with the specific node data (from `what` function), producing a single coherent argument structure for the VFS layer.

Without this module, the RPC layer would lack the specific logic to decode the `MKNOD` procedure's arguments. The complexity of handling the union type and the conditional presence of device numbers would either be pushed into the generic dispatcher (violating separation of concerns) or duplicated in the VFS layer (violating the layering of parsing vs. execution). This module ensures that the nuanced requirements of the NFSv3 protocol regarding special file creation are correctly translated into the VFS layer's type-safe interface.