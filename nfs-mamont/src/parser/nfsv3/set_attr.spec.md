<!-- SPEC_HASH: ea8cd6df295f3bd2217717734cb17809268cb7eabc6170379fa0bc2ffd5ef1f3 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3::set_attr
Rust File: src/parser/nfsv3/set_attr.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io::Read`**: Used as the generic source trait for the `src` parameter in parsing functions. It allows the parser to consume bytes from network streams or test buffers.
- **`crate::parser::nfsv3::create`**: Used to import `new_attr` and `nfs_time`. The `new_attr` function is reused here because the NFSv3 `SETATTR` arguments contain an `sattr3` structure (new attributes) which is identical to the one used in the `CREATE` procedure. The `nfs_time` function is used to parse the timestamp within the `guard` structure.
- **`crate::parser::nfsv3::file`**: Used to import the `handle` function. This is required to parse the file handle of the target object for which attributes are being set.
- **`crate::parser::primitive`**: Used to import the `bool` function. This is required to parse the boolean flag that determines whether the optional `guard` field is present in the wire format.
- **`crate::vfs::set_attr`**: Used to import the `Args` and `Guard` types. These are the target data structures that the parsing functions populate and return.
- **`crate::parser`**: Used to import the `Result` type alias. This ensures that parsing errors are reported consistently with the rest of the parser subsystem.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To deserialize the arguments for the NFSv3 `SETATTR` procedure from a byte stream into the high-level `vfs::set_attr::Args` structure.
- To handle the parsing of the optional `guard` mechanism, which is used for optimistic concurrency control (checking `ctime` before applying changes).
- To reuse the complex attribute parsing logic (`sattr3`) from the `create` module to ensure consistency across different NFS procedures.

Inputs:
- `src: &mut impl Read`: A mutable reference to a byte stream (e.g., network socket or test cursor) from which XDR-encoded data is read.

Outputs:
- `Result<set_attr::Args>`: The fully parsed arguments for the SETATTR procedure.
- `Result<Option<set_attr::Guard>>`: The parsed optional guard structure.

Steps:
1. **`guard` function**:
 - Calls `bool(src)` to read a boolean flag from the stream.
 - If the boolean is `true`, it calls `nfs_time(src)` (imported from `create` module) to read the `ctime` timestamp and wraps it in `Some(Guard)`.
 - If the boolean is `false`, it returns `None`.
2. **`args` function**:
 - Calls `file::handle(src)` to read the file handle identifying the target object.
 - Calls `new_attr(src)` (imported from `create` module) to parse the `sattr3` structure containing the attributes to modify.
 - Calls `guard(src)` to parse the optional verification guard.
 - Aggregates these three components into a `set_attr::Args` struct and returns it.

Edge Cases:
- **Missing Guard**: If the boolean flag for the guard is false, the `ctime` field is absent from the stream, and the parser correctly returns `None` without attempting to read a timestamp.
- **Stream Exhaustion**: If the stream ends prematurely while reading the handle, attributes, or guard, the underlying parsers will return an `Error::IO`, which propagates up.

Complexity:
- Time: O(1) for fixed-size fields (handle, integers). O(N) for variable-length fields inside `new_attr` (e.g., if attributes included variable-length data, though `sattr3` is mostly fixed size with optional fields).
- Space: O(1) for the structures being parsed.

Determinism:
- Deterministic (Given the same input byte stream, the functions produce the same output or error).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::create`**:
 - **`new_attr`**: This mechanism is critical because it encapsulates the logic for parsing the `sattr3` XDR structure. By reusing it, this module avoids duplicating the logic for parsing optional mode, uid, gid, size, and timestamp fields.
 - **`nfs_time`**: This mechanism is used to parse the `nfstime3` structure required for the `guard`'s `ctime` field.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle`**: This mechanism provides the parsing logic for the 64-byte file handle that identifies the target file system object.

- **From `nfs_mamont::parser::primitive`**:
 - **`bool`**: This mechanism handles the parsing of the XDR boolean (encoded as a 32-bit integer) that signifies the presence of the `guard`.

---

## 4. Data Model

Entities:
- This module does not define new public entities. It acts as a transformer, converting byte streams into the entities defined in `nfs_mamont::vfs::set_attr`.

Relations:
- **Transformation**: The functions in this module act as adapters between the `Read` stream and the `vfs::set_attr` structs.
- **Composition**: `set_attr::Args` is composed of a `file::Handle`, a `set_attr::NewAttr`, and an optional `set_attr::Guard`.

Global Invariants:
- **Attribute Structure Consistency**: The `new_attr` field in `Args` must be parsed using the exact same logic as the `sattr3` field in the `CREATE` procedure, which is ensured by importing the function from the `create` module.

## 5. Error Model

Error Types:
- **`parser::Error`**: The primary error type used throughout the module.
 - `Error::IO`: Propagated from the underlying `Read` stream or from dependency parsers if the stream ends unexpectedly.
 - `Error::EnumDiscMismatch`: Propagated from `primitive::bool` if the boolean value is not 0 or 1, or from `create` parsers if discriminants are invalid.

Error Propagation Strategy:
- **Propagation**: The `?` operator is used extensively to propagate errors from `primitive`, `file`, and `create` parsers.

Recoverability:
- **Unrecoverable for the current operation**: If parsing fails, the stream cursor is likely in an undefined state relative to the higher-level RPC message. The caller (typically the RPC dispatcher) must discard the current request.

Panics:
- Allowed: No
- Conditions: The code relies on `Result` propagation from dependencies; no `unwrap` or `expect` calls are present in the parsing logic.

---

## 6. Traits

List which external traits this module implements:
- None.

## 7. Overview

This module is used in order to **deserialize the arguments for the NFSv3 SETATTR procedure** from the network wire format into the structured types required by the server's Virtual File System (VFS). The system contains a parser hierarchy where `primitive` handles raw bytes, `file` handles identifiers, and `vfs::set_attr` defines the semantic interface. This module sits in the middle, implementing the specific logic required to interpret the `SETATTR3args` XDR structure.

A typical usage scenario of the system involves the RPC dispatcher receiving a `SETATTR` request from an NFS client. The dispatcher identifies the procedure number and invokes the `args` function in this module. The function reads the target file handle, then parses the complex set of optional attributes (mode, uid, gid, size, timestamps) by delegating to the `new_attr` function from the `create` module. Finally, it checks for an optional `guard` (containing a `ctime`) which the client uses to verify that the file hasn't been modified by another client since the client last read it. The resulting `set_attr::Args` struct is then passed to the VFS implementation to perform the actual metadata update.

Inside the system, the following things happen and they use this module:
- **Code Reuse**: The module imports `new_attr` from the `create` module. This is a deliberate architectural choice because the NFSv3 specification defines the `sattr3` (set attributes) structure identically for both `CREATE` and `SETATTR` operations. By reusing the parser, the system ensures that any changes to attribute parsing logic automatically apply to both procedures, reducing the risk of divergence.
- **Optimistic Locking Support**: The `guard` function implements the parsing for the `guard` field. This field is crucial for the NFSv3 Weak Cache Consistency model, allowing clients to perform "check and set" operations to avoid race conditions when updating file metadata.

Without this module, the RPC layer would lack a dedicated entry point for parsing `SETATTR` requests. While the logic could theoretically be inlined into a generic dispatcher, the specific structure of `SETATTR` (handle + attributes + guard) and its reliance on the shared `sattr3` parser make it a distinct logical unit that benefits from encapsulation. This module ensures that the nuanced requirements of the `SETATTR` procedure are correctly translated into the VFS layer's type-safe interface.