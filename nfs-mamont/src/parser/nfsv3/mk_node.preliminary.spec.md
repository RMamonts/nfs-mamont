<!-- SPEC_HASH: dc75e207d9e80deb0a2df7a66a8003fd797a0387cff5a48d70138ffcf7a2bbb0 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::mk_node
Rust File: src/parser/nfsv3/mk_node.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the input source (`src`), allowing the parsing functions to consume bytes from any stream (e.g., network buffers).
- **`crate::parser::nfsv3::create`**: This module is used to parse the initial attributes (`NewAttr`) for the special file being created via the `new_attr` function.
- **`crate::parser::nfsv3::file`**: This module is used to parse the directory context of the operation. Specifically, `file::handle` parses the directory file handle, and `file::file_name` parses the name of the new node.
- **`crate::parser::primitive`**: This module provides low-level XDR parsing primitives. It is used to read the `u32` discriminant that determines the type of node to create, as well as the major and minor numbers for device files.
- **`crate::vfs::mk_node`**: This module defines the target data structures (`Args`, `What`) that represent the high-level arguments for the VFS `MkNode` trait. The parser populates these structures.
- **`crate::vfs::file`**: This module defines the `Device` structure, which is used to hold the major and minor numbers parsed for character and block device files.
- **`crate::parser`**: This module provides the `Error` enum and `Result` type alias used for error handling and propagation throughout the parsing logic.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the byte stream of an NFSv3 `MKNOD` procedure call into the structured `mk_node::Args` type used by the VFS layer.
- To interpret the `ftype3` union (represented as `mk_node::What`) which dictates the parsing logic for the remainder of the arguments (attributes vs. device numbers).

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded `MKNOD3args` structure.

Outputs:
- `Result<mk_node::Args>`: A result containing the parsed arguments or a `parser::Error` if the stream is invalid or contains unexpected discriminants.

Steps:
1. **Directory Context Parsing**:
   - Invokes `file::handle(src)` to parse the directory file handle where the node will be created.
   - Invokes `file::file_name(src)` to parse the name of the new node.
   - Wraps these into `vfs::DirOpArgs`.
2. **Node Type Discrimination**:
   - Reads a `u32` discriminant using `primitive::u32(src)` to determine the type of special file.
3. **Variant-Specific Parsing**:
   - **If 3 (Block Device)**: Parses `new_attr(src)`, then reads two `u32` values for major and minor numbers to construct `file::Device`. Returns `What::Block`.
   - **If 4 (Character Device)**: Parses `new_attr(src)`, then reads two `u32` values for major and minor numbers to construct `file::Device`. Returns `What::Char`.
   - **If 6 (Socket)**: Parses `new_attr(src)`. Returns `What::Socket`.
   - **If 7 (Fifo)**: Parses `new_attr(src)`. Returns `What::Fifo`.
   - **Otherwise**: Returns `Error::EnumDiscMismatch`.
4. **Result Construction**: Combines the `DirOpArgs` and the parsed `What` variant into `mk_node::Args` and returns it.

Edge Cases:
- **Invalid Discriminants**: If the type discriminant is not 3, 4, 6, or 7, the function returns `Error::EnumDiscMismatch`.
- **Stream Exhaustion**: If the stream ends prematurely while reading required fields (e.g., attributes or device numbers), the underlying parsers will return an `IO` error, which propagates up.

Complexity:
- Time: O(1) for fixed-size fields (handles, integers, attributes). O(N) for variable-length fields (filenames), where N is the length of the string, due to the dependency on `file::file_name`.
- Space: O(1) for the parsing logic itself, excluding the space allocated for the returned structures.

Determinism:
- Deterministic. Given the same input byte stream, the functions will always produce the same output structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::create`**:
 - `new_attr`: Used to parse the `set_attr::NewAttr` structure. This structure contains optional fields for mode, uid, gid, size, and timestamps, which define the initial state of the special file.
- **From `nfs_mamont::parser::nfsv3::file`**:
 - `handle`: Parses the directory file handle, validating that it matches the expected NFSv3 size.
 - `file_name`: Parses the filename string, enforcing length limits and UTF-8 validity.
- **From `nfs_mamont::parser::primitive`**:
 - `u32`: Used to read the type discriminant and the major/minor numbers for device files, ensuring they are interpreted as Big-Endian integers.
- **From `nfs_mamont::vfs::mk_node`**:
 - `What` enum: Defines the variants (`Block`, `Char`, `Socket`, `Fifo`) that determine the control flow of the parsing logic based on the discriminant.
 - `Args`: The target structure populated by the `args` function.

---

## 4. Data Model

Entities:
- **`mk_node::Args`**: The top-level structure containing the directory context (`DirOpArgs`) and the creation method (`What`).
- **`mk_node::What`**: An enum representing the type of special file to create.
 - `Block(NewAttr, Device)`: Block device.
 - `Char(NewAttr, Device)`: Character device.
 - `Socket(NewAttr)`: Socket.
 - `Fifo(NewAttr)`: Named pipe (FIFO).
- **`file::Device`**: A structure containing `major` and `minor` numbers (u32), used for device files.

Relations:
- **Composition**: `mk_node::Args` contains `vfs::DirOpArgs` and `mk_node::What`.
- **Composition**: `mk_node::What` variants contain `set_attr::NewAttr` and optionally `file::Device`.

Global Invariants:
- The type discriminant must be 3, 4, 6, or 7 to be valid.
- The `Block` and `Char` variants of `What` must always contain a valid `file::Device` with major and minor numbers.

## 5. Error Model

Error Types:
- **`parser::Error`**:
 - `EnumDiscMismatch`: Returned when the type discriminant is not 3, 4, 6, or 7.
 - `IO(io::Error)`: Propagated from the underlying `Read` trait or primitive parsers if the stream is exhausted or fails.

Error Propagation Strategy:
- Custom enum (`Result<T>`). Errors are propagated using the `?` operator. The module explicitly constructs `Error::EnumDiscMismatch` for invalid protocol states.

Recoverability:
- Generally non-recoverable for the specific RPC request. If the arguments cannot be parsed, the request cannot be processed, and the server should typically return an RPC `GarbageArgs` error.

Panics:
- Allowed: No.
- Conditions: The code does not explicitly panic. It relies on `Result` propagation for all failure modes.

---

## 6. Traits

List which external traits this module implements:
- None. This module only defines free-standing parsing functions.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to interpret the `MKNOD` operation arguments sent by an NFSv3 client, translating the raw XDR byte stream into the structured arguments required by the Virtual File System (VFS) layer. The system contains a complex NFS server implementation where the VFS layer is abstracted from the wire format details. A typical usage scenario involves a client requesting to create a special file, such as a character device (e.g., `/dev/null`) or a named pipe (FIFO). The client specifies *what* type of node to create using a specific integer discriminant. This module parses that intent.

Inside the system, the following things happen and they use this module: The RPC dispatcher receives a `MKNOD` request. It invokes the `args` function in this module. The function reads the directory handle and filename to identify *where* to create the file. Then, it reads the type discriminant. If the client requested a device file (Block or Char), the parser reads the specific major and minor numbers required to identify the device driver. If the client requested a Socket or Fifo, it parses only the initial attributes. Without this module, the VFS `MkNode` trait would not receive the necessary context (like the device numbers or initial attributes) to perform the operation correctly according to the NFSv3 specification. This module effectively bridges the gap between the network protocol's representation of special files and the filesystem's implementation of them.