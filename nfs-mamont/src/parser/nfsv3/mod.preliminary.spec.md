<!-- SPEC_HASH: 1654f9e13b1691ce3f958896dbc2c1da58fc0518c1ab5f4de9c9c5494baa58d4 -->
# Module Specification

Module: nfs_mamont::parser::nfsv3
Rust File: src/parser/nfsv3/mod.rs

---

## 1. Dependencies

From *.deps.json and code analysis, for each dependency, write down its purpose — why it is used in this module.

- **`access`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `ACCESS` procedure arguments.
- **`commit`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `COMMIT` procedure arguments.
- **`create`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `CREATE` procedure arguments. It also exports shared attribute parsing logic used by other procedures.
- **`file`**: This module is declared as a public submodule to provide common parsing primitives (file handles, names, paths, attributes) used by almost all other procedure-specific parsers.
- **`fs_info`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `FSINFO` procedure arguments.
- **`fs_stat`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `FSSTAT` procedure arguments.
- **`get_attr`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `GETATTR` procedure arguments.
- **`link`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `LINK` procedure arguments.
- **`lookup`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `LOOKUP` procedure arguments.
- **`mk_dir`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `MKDIR` procedure arguments.
- **`mk_node`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `MKNOD` procedure arguments.
- **`path_conf`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `PATHCONF` procedure arguments.
- **`read`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `READ` procedure arguments.
- **`read_dir`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `READDIR` procedure arguments.
- **`read_dir_plus`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `READDIRPLUS` procedure arguments. It also exports cookie parsing logic reused by `read_dir`.
- **`read_link`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `READLINK` procedure arguments.
- **`remove`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `REMOVE` procedure arguments.
- **`rename`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `RENAME` procedure arguments.
- **`rm_dir`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `RMDIR` procedure arguments.
- **`set_attr`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `SETATTR` procedure arguments. It also exports attribute parsing logic used by other procedures.
- **`symlink`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `SYMLINK` procedure arguments.
- **`write`**: This module is declared as a public submodule to encapsulate the parsing logic for the NFSv3 `WRITE` procedure arguments.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To act as a namespace aggregator for the entire NFSv3 argument parsing subsystem.
- To organize the parsing logic for all NFSv3 RPC procedures into a logical hierarchy, separating concerns by procedure type while keeping them accessible under a common module path.

Inputs:
- None. This module does not define any executable functions or accept runtime inputs.

Outputs:
- None. This module does not return values. It only exports submodules.

Steps:
1. The module declares a series of `pub mod` statements.
2. These statements make the parsing logic defined in the respective submodules (e.g., `access.rs`, `read.rs`) available under the `nfs_mamont::parser::nfsv3` path.

Edge Cases:
- None. This is a structural module.

Complexity:
- Time: N/A.
- Space: N/A.

Determinism:
- N/A.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::parser::nfsv3::file`**:
 - **Common Primitives**: Provides `handle`, `file_name`, `file_path`, `attr`, etc. These are the fundamental building blocks used by almost every other procedure parser to read file identifiers and metadata from the wire format.
- **From Procedure-Specific Modules (e.g., `access`, `read`, `create`)**:
 - **`args` functions**: Each procedure module typically exposes a public `args(src: &mut impl Read) -> Result<ProcedureArgs>` function. This is the primary interface used by the RPC dispatcher to deserialize specific procedure calls.
 - **Shared Logic**: Modules like `create` and `set_attr` provide parsing functions for complex structures (like `sattr3`) that are reused by other modules (e.g., `mk_dir`, `symlink`), ensuring consistency across the protocol implementation.

---

## 4. Data Model

Entities:
- **Submodules**: The module aggregates the following logical entities:
 - Procedure Parsers: `access`, `commit`, `create`, `fs_info`, `fs_stat`, `get_attr`, `link`, `lookup`, `mk_dir`, `mk_node`, `path_conf`, `read`, `read_dir`, `read_dir_plus`, `read_link`, `remove`, `rename`, `rm_dir`, `set_attr`, `symlink`, `write`.
 - Utility Parsers: `file` (common types), `create` (attribute structures).

Relations:
- **Namespace Hierarchy**: The parent module `nfsv3` owns the submodules, providing a scoped path `nfs_mamont::parser::nfsv3::<procedure>` to access specific parsers.

Global Invariants:
- All submodules are public, meaning the entire NFSv3 parsing interface is exposed to the rest of the `nfs_mamont` crate (and potentially external users if the crate is a library).

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

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to provide a unified, hierarchical namespace for the entire NFSv3 protocol argument parsing subsystem within the `nfs_mamont` server. The system contains a large number of distinct RPC procedures (over 20), each requiring a unique binary layout to be deserialized from the network stream. Without this parent module, these parsers would be scattered or flatly organized, making the codebase difficult to navigate and manage. This module acts as the root of the "NFSv3 Parser" domain, grouping related functionality and separating it from other protocol versions (e.g., NFSv4) or other parsing concerns.

A typical usage scenario of the system involves the RPC dispatcher receiving a request. The dispatcher identifies the protocol version as NFSv3 and the specific procedure number (e.g., `READ`). It then imports the parser from this module hierarchy (e.g., `use nfs_mamont::parser::nfsv3::read::args`) and invokes it. This organization allows the dispatcher to remain protocol-agnostic in its routing logic while delegating the specific wire-format interpretation to the appropriate submodule.

Inside the system, the following things happen and they use this module:
- **Logical Grouping**: The module organizes parsers into a coherent structure. For example, `file` is placed at the root level because it provides utilities used by almost all other procedures, while `read` and `write` are separate because they handle distinct operations.
- **Shared Dependency Management**: By centralizing modules like `create` and `file` here, the system ensures that common logic (like parsing file attributes or creation modes) is defined in one place and reused by `mk_dir`, `symlink`, `create`, and `mk_node`, preventing code duplication and ensuring protocol consistency.
- **Interface Stability**: This module defines the public API boundary for the NFSv3 parser. Any external component needing to parse NFSv3 arguments interacts with this module path, abstracting away the internal file structure of the parser implementation.