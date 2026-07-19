<!-- SPEC_HASH: 3dce288def7d104666824f909824d3c4567691d63e8acf71329413e43e6f64d9 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::mk_dir
Rust File: src/parser/nfsv3/mk_dir.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the source trait for the `src` parameter in the `args` function. It allows the parser to consume bytes from a generic stream (e.g., network socket or test buffer).
- **`crate::parser::nfsv3::create`**: Used to import the `new_attr` function. This function is reused here to parse the `sattr3` (set attributes) structure from the wire format, which defines the initial attributes for the new directory. This avoids code duplication as the attribute structure is identical to the one used in the `CREATE` procedure.
- **`crate::parser::nfsv3::file`**: Used to import `handle` and `file_name`. These functions parse the parent directory's file handle and the name of the new directory, respectively, forming the `DirOpArgs` part of the request.
- **`crate::vfs::mk_dir`**: Used to import the `Args` structure. This is the target type that the `args` function constructs and returns, representing the fully parsed arguments for the VFS layer.
- **`crate::parser`**: Used to import the `Result` type alias. This ensures that parsing errors are reported consistently using the `parser::Error` enum.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `MKDIR` procedure from a byte stream into the `vfs::mk_dir::Args` structure.
- To compose the directory context (parent handle and name) with the initial attributes by reusing the attribute parsing logic from the `create` module.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream containing the XDR-encoded `MKDIR3args` structure.

Outputs:
- `Result<mk_dir::Args>`: A Result containing the parsed arguments or a `parser::Error`.

Steps:
1. **Parse Directory Context**: The function calls `file::handle(src)` to read the file handle of the parent directory and `file::file_name(src)` to read the name of the directory to be created.
2. **Construct Directory Arguments**: The parsed handle and name are wrapped in a `vfs::DirOpArgs` struct.
3. **Parse Attributes**: The function calls `new_attr(src)` (imported from the `create` module) to parse the optional initial attributes (`sattr3`) for the new directory.
4. **Construct Result**: The `vfs::DirOpArgs` and the parsed `set_attr::NewAttr` are aggregated into the final `mk_dir::Args` struct, which is then wrapped in `Ok` and returned.

Edge Cases:
- **Stream Exhaustion**: If the byte stream ends prematurely while reading the handle, name, or attributes, the underlying calls to `file::handle`, `file::file_name`, or `new_attr` will return an `Error::IO`, which is propagated up.
- **Invalid Data**: If the handle size is incorrect, the name is invalid, or the attribute discriminants are unknown, the specific parsing functions will return the corresponding `parser::Error` (e.g., `BadFileHandle`, `EnumDiscMismatch`).

Complexity:
- Time: O(N) where N is the length of the directory name string. Parsing the handle and attributes is O(1) as they have fixed or bounded sizes.
- Space: O(N) for the allocated string representing the directory name. Other structures are stack-allocated.

Determinism:
- Deterministic. Given the same input byte stream, the function will always produce the same `mk_dir::Args` structure or the same error.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle`**: This mechanism is used to read the 64-byte file handle identifying the parent directory. It ensures the handle conforms to `NFS3_FHSIZE`.
 - **`file_name`**: This mechanism is used to read the variable-length string for the new directory name. It enforces `MAX_NAME_LEN` and validates that the name does not contain invalid characters (like slashes).

- **From `nfs_mamont::parser::nfsv3::create`**:
 - **`new_attr`**: This mechanism is critical for parsing the `sattr3` structure. It handles the XDR logic for optional fields (mode, uid, gid, size, atime, mtime), allowing the client to specify initial properties for the directory. By reusing this function, the `mk_dir` parser ensures that attribute handling is consistent with the `CREATE` procedure.

---

## 4. Data Model

Entities:
- This module does not define any new public entities. It acts as a transformer, converting a byte stream into the `mk_dir::Args` entity defined in `nfs_mamont::vfs::mk_dir`.

Relations:
- **Transformation**: The `args` function transforms the `Read` stream into a `mk_dir::Args` struct.
- **Composition**: The `mk_dir::Args` struct is composed of a `vfs::DirOpArgs` (created from `file::Handle` and `file::Name`) and a `set_attr::NewAttr` (created by `new_attr`).

Global Invariants:
- **Wire Format Order**: The parser implicitly relies on the NFSv3 specification order: directory handle, followed by directory name, followed by the attribute structure.

## 5. Error Model

Error Types:
- **`parser::Error`**: The error type propagated from the underlying parsing functions.
 - `Error::IO`: Propagated from `Read` operations or string validation.
 - `Error::BadFileHandle`: Propagated from `file::handle` if the size is incorrect.
 - `Error::EnumDiscMismatch`: Propagated from `new_attr` if an attribute set-time discriminant is invalid.
 - `Error::MaxElemLimit`: Propagated from `file::file_name` if the name is too long.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used to immediately return any error encountered during the parsing of the handle, name, or attributes.

Recoverability:
- **Unrecoverable for the current operation**: If parsing fails, the stream cursor is in an undefined state relative to the RPC message boundaries. The caller must discard the current request.

Panics:
- Allowed: No.
- Conditions: The code consists entirely of calls to fallible parsing functions and struct construction. No `unwrap`, `expect`, or indexing operations that could panic are present.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **interpret the specific binary format of the NFSv3 `MKDIR` procedure arguments** and translate them into the high-level structures used by the server's Virtual File System (VFS). The system contains a parser hierarchy where low-level primitives handle bytes and mid-level modules handle specific structures. This module sits at the procedure-specific layer, orchestrating the parsing of the `MKDIR3args` union.

A typical usage scenario of the system involves the RPC dispatcher receiving a request with a procedure number indicating `MKDIR`. The dispatcher invokes the `args` function in this module. The function reads the parent directory handle and the new name, then parses the optional attributes (like permissions or ownership) that the client wishes to apply immediately upon creation. The resulting `mk_dir::Args` struct is then passed to the VFS backend to execute the actual directory creation logic.

Inside the system, the following things happen and they use this module:
- **Code Reuse**: The module leverages the `new_attr` function from the `create` module. This is significant because the NFSv3 protocol defines the `sattr3` (set attributes) structure identically for both creating files and creating directories. By delegating to `create::new_attr`, this module avoids duplicating the complex logic for parsing optional fields and timestamp settings.
- **Context Assembly**: The module combines the "where" (parent handle + name) and the "what" (attributes) into a single `Args` object. This object is the precise input required by the `vfs::mk_dir::MkDir` trait, ensuring a clean handoff between the network parsing layer and the storage logic layer.

Without this module, the RPC layer would lack a dedicated entry point for parsing `MKDIR` requests, forcing it to inline this logic or rely on a generic, less type-safe mechanism. This module encapsulates the specific wire-format requirements of the `MKDIR` procedure, ensuring that the VFS layer receives valid, structured data.