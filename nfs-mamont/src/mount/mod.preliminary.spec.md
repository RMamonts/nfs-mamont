<!-- SPEC_HASH: afece1e62a20e4e22bb246625b42824d484bc6285f4f9060bdff34876893d580 -->
# Module Specification

Module: nfs_mamont::mount
Rust File: src/mount/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::consts::mount`**: Used to access the `MOUNT_HOST_NAME_LEN` constant. This constant defines the maximum allowed length for a client host name string, which is enforced by the `HostName` constructor.
- **`crate::vfs::file`**: Used to import the `file::Path` type. This type is utilized within `MountEntry` and `ExportEntry` to represent server-side directory paths in a validated, type-safe manner consistent with the VFS layer.
- **`std::io`**: Used to provide the `io::Result` type and `io::Error` struct. These are used in the `HostName::new` constructor to return validation errors if the input string exceeds the maximum allowed length.
- **Sub-modules (`dump`, `export`, `mnt`, `umnt`, `umntall`)**: These modules are declared as public sub-modules. The current module aggregates their interfaces (traits) and data types to form a cohesive implementation of the MOUNT protocol. The `Mount` trait defined here acts as a super-trait combining the specific procedure traits defined in these sub-modules.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the shared data structures and the unified service interface for the NFS MOUNT protocol version 3.
- To enforce protocol-level constraints on client identifiers (hostnames) at the type level.
- To aggregate the specific procedure traits (MNT, UMNT, UMNTALL, EXPORT, DUMP) into a single `Mount` trait, facilitating dependency injection and service registration.

Inputs:
- `name: String`: Raw string input for constructing a `HostName`.

Outputs:
- `io::Result<HostName>`: A validated hostname wrapper or an error if the name is too long.
- `MountEntry`: A record linking a client hostname to a mounted directory path.
- `ExportEntry`: A record linking a directory path to a list of allowed client hostnames.
- `MountRes`: An enum wrapping the result bodies of various MOUNT procedures.

Steps:
1. **Hostname Validation**: When `HostName::new` is called, the length of the input string is compared against `MOUNT_HOST_NAME_LEN`.
2. **Error Handling**: If the length exceeds the limit, an `io::Error` with kind `InvalidInput` is returned immediately.
3. **Instantiation**: If valid, the string is wrapped in the `HostName` struct.
4. **Trait Composition**: The `Mount` trait is defined as a combination of `mnt::Mnt`, `umnt::Umnt`, `umntall::Umntall`, `export::Export`, and `dump::Dump`. A blanket implementation is provided for any type `T` that implements all these sub-traits.

Edge Cases:
- **Hostname Length**: A hostname string exactly matching `MOUNT_HOST_NAME_LEN` is accepted; any longer string is rejected.
- **Empty Hostname**: The code does not explicitly check for empty strings, only length. (Assumption: An empty string is valid if length <= limit, though semantically unlikely in practice).

Complexity:
- **Time**: O(1) for `HostName::new` (length check is constant time relative to protocol limits, though technically O(N) on string length).
- **Space**: O(1) for `HostName` (wrapper), O(N) for `MountEntry`/`ExportEntry` (depending on path/vec size).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
    - **Path Validation**: The `MountEntry` and `ExportEntry` structs rely on `file::Path` to ensure that directory paths are valid and conform to VFS constraints before being used in MOUNT protocol records.
- **From `nfs_mamont::consts::mount`**:
    - **Protocol Limits**: The `MOUNT_HOST_NAME_LEN` constant provides the hard boundary for the `HostName` validation logic, ensuring interoperability with the NFS MOUNT specification.
- **From Sub-modules (`mnt`, `umnt`, `umntall`, `export`, `dump`)**:
    - **Procedure Interfaces**: The `Mount` trait aggregates the specific asynchronous procedure interfaces defined in these modules. This allows the `nfs_mamont` server to treat the MOUNT service as a single object that can handle all related RPCs.

---

## 4. Data Model

Entities:
- **`HostName`**: A wrapper around a `String` representing a client's host name. It guarantees that the inner string does not exceed the protocol-defined maximum length.
- **`MountEntry`**: A structure representing an active mount. It consists of a `HostName` (client) and a `file::Path` (server directory).
- **`ExportEntry`**: A structure representing an exported filesystem. It consists of a `file::Path` (directory) and a `Vec<HostName>` (list of clients allowed to mount it).
- **`MountRes`**: An enumeration serving as a wrapper for the result bodies of the MOUNT protocol procedures. It contains variants for `Null`, `Mount`, `Unmount`, `Export`, `Dump`, and `UnmountAll`.

Relations:
- **`MountEntry` → `HostName` (Composition)**: A mount entry owns a hostname.
- **`MountEntry` → `file::Path` (Composition)**: A mount entry owns a directory path.
- **`ExportEntry` → `file::Path` (Composition)**: An export entry owns a directory path.
- **`ExportEntry` → `Vec<HostName>` (Aggregation)**: An export entry contains a list of hostnames.

Global Invariants:
- The inner string of a `HostName` instance must have a length less than or equal to `MOUNT_HOST_NAME_LEN`.

## 5. Error Model

Error Types:
- **`std::io::Error`**: Specifically used with `io::ErrorKind::InvalidInput`.

Error Propagation Strategy:
- **Direct Return**: The `HostName::new` constructor returns `io::Result<Self>`. Errors are propagated directly to the caller.

Recoverability:
- **Recoverable**: The caller can catch the `io::Error` and handle the invalid hostname (e.g., by rejecting the client request).

Panics:
- **Allowed**: No
- **Conditions**: The public API does not perform any operations that could panic (like indexing or unwrapping) other than the standard length check.

---

## 6. Traits

List which external traits this module implements:
- **`Mount`**: A super-trait defined in this module. It combines `mnt::Mnt`, `umnt::Umnt`, `umntall::Umntall`, `export::Export`, and `dump::Dump`. It has a blanket implementation for any type `T` that satisfies all these bounds.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **provide a unified type system and service interface for the NFS MOUNT protocol**, acting as the root of the MOUNT subsystem. The system contains a complex implementation of an NFS server where the MOUNT protocol is responsible for the initial handshake: mapping client-requested paths to server file handles and managing access control lists (exports).

A typical usage scenario of the system involves the RPC dispatcher receiving a MOUNT protocol request. The dispatcher needs a single service object to handle all MOUNT procedures (MNT, UMNT, EXPORT, etc.). This module provides the `Mount` trait, which aggregates the specific procedure traits defined in the sub-modules. Furthermore, when the server needs to record that a client has mounted a directory, it uses the `MountEntry` struct defined here to store the association between the client's `HostName` and the `file::Path`. When configuring which directories are available to clients, the server uses `ExportEntry`.

Inside the system, the following things happen and they use this module:
1.  **Type Safety**: The `HostName` struct ensures that any client identifier stored or transmitted adheres to the RFC 1813 length limits, preventing buffer overflows or protocol violations.
2.  **Service Composition**: The `Mount` trait allows the server implementation to define a single struct that implements all necessary MOUNT procedures. The RPC layer can then hold a reference to `dyn Mount` (or a generic `M: Mount`) and dispatch requests without needing to know about the individual procedure traits.
3.  **Data Transfer**: The `MountRes` enum (though marked `dead_code` in the snippet, it is public) suggests a mechanism for unifying the return types of different procedures, potentially used in internal routing or generic RPC handling layers.

Without this module, the MOUNT protocol implementation would lack a centralized definition of its core data structures (like what constitutes a valid hostname or a mount record), leading to potential inconsistencies between the different procedure implementations (e.g., `mnt` and `umnt` using different definitions of a client identifier).