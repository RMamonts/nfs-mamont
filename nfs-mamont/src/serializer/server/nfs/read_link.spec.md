<!-- SPEC_HASH: e80b142f3aeafec5a19ba79221d3053833201a291e1c83f932f8ccb4490db99c -->
# Module Specification

Module: nfs_mamont::serializer::server::nfs::read_link
Rust File: src/serializer/server/nfs/read_link.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **std::io**: Used to provide the `Write` trait, which defines the interface for the destination byte stream (e.g., a network buffer), and `io::Result` for handling I/O errors during serialization.
- **crate::serializer::files**: Used to import `file_attr` and `file_path`. These functions are responsible for the low-level XDR serialization of file metadata and path strings, respectively, which are the components of the `READLINK` response.
- **crate::serializer**: Used to import the `option` function. This helper serializes `Option<T>` types according to XDR rules (writing a boolean discriminator followed by the value if present), which is required for the optional `symlink_attr` field in the response structures.
- **crate::vfs::read_link**: Used to import the `Success` and `Fail` structs. These represent the high-level VFS output of the `READLINK` operation and serve as the input data structures for the serialization functions defined in this module.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To convert the Virtual File System (VFS) result types for the `READLINK` procedure (`Success` and `Fail`) into the binary XDR (External Data Representation) format specified by the NFSv3 protocol (RFC 1813).
- To ensure that the optional post-operation attributes (`symlink_attr`) and the link data (`data`) are written to the output stream in the correct order and format.

Inputs:
- `dest`: A mutable reference to a type implementing `std::io::Write`, acting as the byte sink.
- `arg`: Either `read_link::Success` (containing optional attributes and the link path) or `read_link::Fail` (containing optional attributes).

Outputs:
- `io::Result<()>`: Indicates successful writing of the XDR encoded response to the destination or an error if the underlying write operation fails.

Steps:
1. **Serialization of `result_ok`**:
   - The function calls `option` with `arg.symlink_attr`. If `Some`, it invokes the closure `|attr, dest| file_attr(dest, &attr)` to serialize the file attributes. If `None`, it writes a `false` boolean discriminator.
   - The function calls `file_path` with `arg.data` to serialize the target path of the symbolic link as an XDR string.
2. **Serialization of `result_fail`**:
   - The function calls `option` with `arg.symlink_attr`. Similar to `result_ok`, it conditionally serializes the file attributes if they are present. Note that `result_fail` does not serialize link data, as per the NFSv3 specification for failed `READLINK` responses.

Edge Cases:
- **Missing Attributes**: If `symlink_attr` is `None` in either `Success` or `Fail`, the `option` function ensures that only the `false` discriminator is written, omitting the attribute data.
- **Invalid Path Data**: If `arg.data` in `Success` contains invalid UTF-8 or exceeds protocol limits, the `file_path` function (from the dependency) will return an `io::Error`, which propagates through `result_ok`.

Complexity:
- Time: O(N), where N is the size of the serialized data (attributes + path length).
- Space: O(1) auxiliary space (streaming directly to the writer).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer::files`**:
 - **`file_attr`**: Used to serialize the `file::Attr` struct contained within `symlink_attr`. It handles the conversion of complex metadata (mode, size, timestamps, etc.) into the XDR `fattr3` format.
 - **`file_path`**: Used to serialize the `file::Path` struct contained in `Success.data`. It converts the internal path representation to a UTF-8 string and writes it as an XDR variable-length string with appropriate padding.

- **From `nfs_mamont::serializer`**:
 - **`option`**: Used to handle the `Option<file::Attr>` fields. It abstracts the XDR logic for optional data (writing a boolean discriminant `0` or `1` followed by the data if `1`), ensuring the protocol's union requirements are met for the `attributes` field in both `READLINK3resok` and `READLINK3resfail`.

- **From `nfs_mamont::vfs::read_link`**:
 - **`Success` and `Fail`**: These structs provide the data contract. `Success` guarantees the presence of `data` (the link target) and optional `symlink_attr`. `Fail` provides optional `symlink_attr` (for cache consistency even on failure) but no data.

---

## 4. Data Model

Entities:
- This module defines no new entities. It operates on `read_link::Success` and `read_link::Fail` defined in `vfs::read_link`.

Relations:
- **Mapping**: `result_ok` maps `read_link::Success` to the XDR `READLINK3resok` structure. `result_fail` maps `read_link::Fail` to the XDR `READLINK3resfail` structure.

Global Invariants:
- The output byte stream must conform to the NFSv3 XDR specification for `READLINK` responses. Specifically, `result_ok` must output the optional attributes followed by the link data, while `result_fail` must output only the optional attributes.

## 5. Error Model

Error Types:
- `std::io::Error`

Error Propagation Strategy:
- Errors are propagated using the `?` operator. If `option`, `file_attr`, or `file_path` return an `Err`, it is immediately returned to the caller.

Recoverability:
- Recoverable. The caller receives the `io::Result` and can decide how to handle the serialization failure (e.g., by aborting the RPC connection).

Panics:
- Allowed: No
- Conditions: The code relies on the `Write` trait and `Result` handling; no explicit panics are triggered by this module.

---

## 6. Traits

List which external traits this module implements:
- None. This module defines free-standing serialization functions.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
You MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to serialize the specific results of the NFSv3 `READLINK` procedure into the XDR wire format required for network transmission. The system contains a Virtual File System (VFS) layer that generates high-level Rust structs representing the outcome of file system operations. However, these internal structs cannot be sent directly to an NFS client because they do not adhere to the strict binary layout, byte ordering, and padding rules defined by the XDR standard (RFC 1813).

A typical usage scenario of the system involves an NFS client requesting to read the target of a symbolic link. The VFS layer processes this request and returns a `read_link::Success` struct containing the link's target path and its post-operation attributes. The RPC layer, responsible for sending the response, invokes the `result_ok` function from this module. This function translates the Rust struct into a byte stream: it first writes the optional attributes (using the `option` helper) and then writes the path string (using `file_path`). If the operation failed, `result_fail` is invoked to serialize only the optional attributes, as per the protocol specification.

Inside the system, this module serves as the specific adapter for the `READLINK` procedure. While the `serializer::files` module provides generic tools for serializing files and paths, this module composes those tools to match the exact structure of the `READLINK3resok` and `READLINK3resfail` XDR unions. Without this module, the server would lack the logic to format `READLINK` responses correctly, leading to protocol violations and communication failures with NFS clients expecting the standard format.