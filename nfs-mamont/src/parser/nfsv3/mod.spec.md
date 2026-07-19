<!-- SPEC_HASH: 1654f9e13b1691ce3f958896dbc2c1da58fc0518c1ab5f4de9c9c5494baa58d4 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3
Rust File: src/parser/nfsv3/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **Uncertainty**: The `*.deps.json` file for this specific module was not provided in the context. Based on the source code analysis, this module does not contain any `use` statements or external dependencies. It acts purely as a parent module.
- **Submodules**: The module declares a list of public submodules (`access`, `commit`, `create`, `file`, `fs_info`, `fs_stat`, `get_attr`, `link`, `lookup`, `mk_dir`, `mk_node`, `path_conf`, `read`, `read_dir`, `read_dir_plus`, `read_link`, `remove`, `rename`, `rm_dir`, `set_attr`, `symlink`, `write`). These submodules contain the actual parsing logic.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a unified namespace for the NFSv3 protocol argument parsing subsystem.
- To organize the implementation details of the ~22 distinct NFSv3 procedures into a logical hierarchy, separating them from other protocol versions or generic parsing logic.

Inputs:
- None. This module is a declaration file only.

Outputs:
- A module namespace `nfs_mamont::parser::nfsv3` that exposes the public interfaces of all procedure-specific argument parsers.

Steps:
1. The module declares a series of `pub mod` statements.
2. The Rust compiler resolves these declarations to the corresponding files in the file system (e.g., `access.rs`, `commit.rs`).
3. The public functions within those submodules (e.g., `access::args`, `read::args`) become accessible via the path `nfs_mamont::parser::nfsv3::<procedure>::args`.

Edge Cases:
- **Missing Submodule**: If a submodule file is missing or fails to compile, the entire `nfsv3` module and potentially the parent crate will fail to compile.

Complexity:
- Time: N/A (Compile-time organization).
- Space: N/A.

Determinism:
- Deterministic. The module structure is fixed at compile time.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

Since this module aggregates the submodules, the key mechanisms are the public interfaces provided by each submodule:

- **From `nfs_mamont::parser::nfsv3::access`**:
 - **`args` function**: Provides the entry point for deserializing `ACCESS` procedure arguments.
- **From `nfs_mamont::parser::nfsv3::commit`**:
 - **`args` function**: Provides the entry point for deserializing `COMMIT` procedure arguments.
- **From `nfs_mamont::parser::nfsv3::create`**:
 - **`args` function**: Provides the entry point for deserializing `CREATE` procedure arguments.
 - **`new_attr` function**: Provides a shared utility for parsing attribute structures, reused by other modules like `mk_dir` and `symlink`.
- **From `nfs_mamont::parser::nfsv3::file`**:
 - **`handle`, `file_name`, `attr`, etc.**: Provides fundamental parsing primitives for file handles, names, and attributes, used by almost all other procedure parsers.
- **From `nfs_mamont::parser::nfsv3::lookup`**:
 - **`args` function**: Provides the entry point for deserializing `LOOKUP` procedure arguments.
- **From `nfs_mamont::parser::nfsv3::read`**:
 - **`args` function**: Provides the entry point for deserializing `READ` procedure arguments.
- **From `nfs_mamont::parser::nfsv3::read_dir_plus`**:
 - **`cookie`, `cookie_verifier` functions**: Provide shared parsing logic for directory state tracking, reused by `read_dir`.
- **From `nfs_mamont::parser::nfsv3::remove`**:
 - **`args` function**: Provides the entry point for deserializing `REMOVE` procedure arguments.
- **From `nfs_mamont::parser::nfsv3::rename`**:
 - **`args` function**: Provides the entry point for deserializing `RENAME` procedure arguments.
- **From `nfs_mamont::parser::nfsv3::write`**:
 - **`args` function**: Provides the entry point for deserializing `WRITE` procedure arguments (specifically the metadata part).
- **(And similarly for all other listed submodules)**.

---

## 4. Data Model

Entities:
- This module does not define any public entities. It acts as a container for the entities defined in its submodules.

Relations:
- **Namespace Hierarchy**: This module is the parent of all procedure-specific parser modules (e.g., `nfs_mamont::parser::nfsv3::read` is a child of `nfs_mamont::parser::nfsv3`).

Global Invariants:
- **Completeness**: The module implies that the full set of NFSv3 procedures supported by the server is represented by the list of submodules. If a procedure is supported, a corresponding submodule is expected here.

## 5. Error Model

Error Types:
- None defined in this module.

Error Propagation Strategy:
- N/A.

Recoverability:
- N/A.

Panics:
- Allowed: N/A.
- Conditions: N/A.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- None.

---

## 7. Overview

This module is used in order to **organize the argument parsing logic for the entire NFSv3 protocol** into a distinct, logical namespace within the larger `nfs_mamont` parser subsystem. The system contains a complex RPC server that must support multiple file system protocols (potentially NFSv3, NFSv4, etc.) and a large number of distinct procedures within each protocol. This module serves as the root of the NFSv3 specific implementation, isolating it from other protocol versions and from the generic RPC dispatching logic.

A typical usage scenario of the system involves the RPC dispatcher receiving a request. The dispatcher identifies the protocol version as NFSv3 and the procedure number (e.g., `READ` is procedure 6). The dispatcher uses the path `nfs_mamont::parser::nfsv3::read::args` to locate the specific function responsible for deserializing the bytes for that procedure. Without this parent module, all these parsers would be siblings in a flat `parser` directory, leading to namespace collisions (e.g., `read::args` vs `write::args` might conflict if not namespaced) and poor code organization.

Inside the system, the following things happen and they use this module:
- **Protocol Isolation**: By grouping all NFSv3 logic under `nfsv3`, the system ensures that changes to NFSv4 or other protocols do not interfere with the stable NFSv3 implementation.
- **Shared Resource Access**: Submodules within this namespace (like `file` and `create`) expose public functions that are reused by other submodules (e.g., `mk_dir` uses `create::new_attr`). The parent module facilitates this "sibling" access by placing them in the same scope.
- **Discoverability**: Developers and the compiler can easily locate the implementation of any specific NFSv3 procedure by navigating this module tree.

Without this module, the parser architecture would lack a clear boundary for the NFSv3 protocol, making the codebase harder to navigate, maintain, and extend as new procedures are added or protocol versions are updated. This module provides the necessary structural context for the NFSv3 parsing domain.