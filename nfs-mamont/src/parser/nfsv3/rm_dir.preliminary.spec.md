<!-- SPEC_HASH: 2123b5dfa9455b8a95b41304fd9fc9b3d91b8dd2b1931d50813ae56e742944d5 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::rm_dir
Rust File: src/parser/nfsv3/rm_dir.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: This trait is used as a generic bound for the input source (`src`), allowing the parser to consume bytes from any stream (e.g., network socket, memory buffer).
- **`crate::parser::nfsv3::file`**: This module provides the specific parsing primitives required to extract the components of the `RMDIR` arguments from the byte stream.
 - **`file::handle`**: Used to parse the file handle of the parent directory from the stream.
 - **`file::file_name`**: Used to parse the name of the directory to be removed from the stream.
- **`crate::vfs::rm_dir`**: This module defines the target data structure `Args` which the `args` function populates and returns.
- **`crate::parser`**: This module provides the `Result` type alias used for error handling throughout the parsing logic.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the specific wire format of the NFSv3 `RMDIR` procedure arguments into the structured `rm_dir::Args` type used by the VFS layer. This involves extracting a directory handle and a directory name sequentially from the input stream.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the serialized `RMDIR` arguments in NFSv3/XDR format.

Outputs:
- `Result<rm_dir::Args>`: A result containing the populated `rm_dir::Args` structure on success, or a `parser::Error` if the stream is malformed or contains invalid data.

Steps:
1. **Parse Directory Handle**: The function calls `file::handle(src)` to read the file handle of the parent directory from the stream. This operation consumes the necessary bytes and validates the handle structure.
2. **Parse Directory Name**: The function calls `file::file_name(src)` to read the name of the subdirectory to be removed. This operation consumes the length-prefixed string and validates its length and content.
3. **Construct Arguments**: The function constructs a `vfs::DirOpArgs` structure using the parsed handle and name.
4. **Wrap and Return**: The `vfs::DirOpArgs` is wrapped inside `rm_dir::Args` (specifically in the `object` field), which is then returned wrapped in `Ok`.

Edge Cases:
- **Malformed Handle**: If the file handle in the stream does not meet the protocol requirements (e.g., incorrect length), `file::handle` will return an error, which propagates immediately.
- **Invalid Name**: If the directory name is too long or contains invalid characters, `file::file_name` will return an error, which propagates immediately.
- **Unexpected EOF**: If the stream ends prematurely during either the handle or name parsing, an `IO` error from the underlying read operations will propagate.

Complexity:
- Time: O(N), where N is the total size of the file handle and the directory name in bytes.
- Space: O(N), primarily for storing the file handle bytes and the directory name string.

Determinism:
- Deterministic. Given the same input byte sequence, the function will always produce the same `rm_dir::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle`**: Reads a length-prefixed byte array representing a file handle. It ensures the length matches `NFS3_FHSIZE` (64 bytes) and returns a `file::Handle` or `Error::BadFileHandle`.
 - **`file_name`**: Reads a length-prefixed string representing a filename. It enforces maximum length constraints (`MAX_NAME_LEN`) and returns a `file::Name` or `Error::MaxElemLimit`/`Error::IO`.
- **From `nfs_mamont::vfs::rm_dir`**:
 - **`Args`**: The structure being populated. It acts as a container for `vfs::DirOpArgs`, which groups the directory handle and the entry name into a single logical unit representing the operation's target.

---

## 4. Data Model

Entities:
- **`rm_dir::Args`**: The final output structure representing the arguments for the `RMDIR` VFS operation.
- **`vfs::DirOpArgs`**: An intermediate structure (defined in `vfs`) that holds the directory handle and the name.
- **`file::Handle`**: The unique identifier for the parent directory.
- **`file::Name`**: The name of the subdirectory to be removed.

Relations:
- `rm_dir::Args` **contains** `vfs::DirOpArgs` (1:1).
- `vfs::DirOpArgs` **contains** `file::Handle` (1:1) and `file::Name` (1:1).

Global Invariants:
- The input stream `src` must contain the file handle data immediately followed by the file name data. No other data or padding is expected between these two components in the context of this parser.

---

## 5. Error Model

Error Types:
- **`parser::Error`**: This module does not define its own errors but propagates those from its dependencies.
 - `Error::BadFileHandle`: Propagated from `file::handle` if the handle length is invalid.
 - `Error::MaxElemLimit`: Propagated from `file::file_name` if the name is too long.
 - `Error::IO`: Propagated if the stream ends unexpectedly or other read errors occur.

Error Propagation Strategy:
- **Custom Result Type**: Uses `crate::parser::Result<T>`. Errors are propagated implicitly using the `?` operator on the sub-parsing calls (`file::handle` and `file::file_name`).

Recoverability:
- Non-recoverable for the specific parsing operation. If parsing fails, the stream state is undefined (partially consumed), and the caller must abort the processing of the current RPC request.

Panics:
- Allowed: No.
- Conditions: The code consists solely of calls to fallible parsing functions wrapped in `Ok` and `?`. No `unwrap`, `expect`, or `panic!` calls are present.

---

## 6. Traits

List which external traits this module implements:
- None. This module only defines a free-standing function.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to translate the raw byte stream of an incoming NFSv3 `RMDIR` RPC request into the structured `Args` object required by the Virtual File System (VFS) to execute the operation. The system contains a network stack that receives binary data; without this specific parser module, the VFS layer would not be able to interpret the sequence of bytes representing the target directory handle and the name of the directory to be removed.

A typical usage scenario of the system involves the NFS server receiving an `RMDIR` request packet. The RPC dispatcher identifies the procedure number and invokes the corresponding parser function—in this case, `args`. The `args` function reads the bytes to extract the parent directory's file handle and the specific directory name. It validates these components (ensuring the handle size is correct and the name length is within limits) and packages them into `rm_dir::Args`. This structured argument is then passed to the `Vfs::rm_dir` trait method, which performs the actual file system logic (checking permissions, verifying the directory is empty, and removing it).

Inside the system, the following things happen and they use this module:
- **Protocol Decoding**: The `args` function acts as the entry point for decoding the `RMDIR` specific arguments, ensuring the data adheres to the NFSv3 wire format (Handle followed by Name).
- **Validation Delegation**: Instead of implementing validation logic itself, this module delegates the detailed validation of the handle and name to the `file` module, ensuring consistency across different NFS procedures that use these common types.
- **Type Safety**: By converting raw bytes into `rm_dir::Args`, the module allows the rest of the server code to work with strongly-typed Rust structs, preventing type errors and ensuring that all necessary fields for the operation are present.