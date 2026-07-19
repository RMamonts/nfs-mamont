<!-- SPEC_HASH: afece1e62a20e4e22bb246625b42824d484bc6285f4f9060bdff34876893d580 -->
# Module Specification

Module: nfs_mamont::mount
Rust File: src/mount/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::io`**: Used to provide the `io::Result` type and `io::Error` struct. The `HostName::new` constructor returns `io::Result` to signal validation failures (specifically when the hostname exceeds the maximum allowed length) using the `InvalidInput` error kind.
- **`crate::consts::mount`**: Used to import the constant `MOUNT_HOST_NAME_LEN`. This constant defines the maximum byte length for a client hostname string, which is enforced by the `HostName` wrapper to ensure compliance with the MOUNT protocol limits.
- **`crate::vfs::file`**: Used to import the `file::Path` type. This type is utilized in `MountEntry` and `ExportEntry` to represent directory paths, ensuring that the paths used in the MOUNT protocol adhere to the VFS layer's validation rules (e.g., length limits, non-emptiness).
- **Sub-modules (`dump`, `export`, `mnt`, `umnt`, `umntall`)**: These modules are declared as public children of this module. They define the specific traits and data structures for individual MOUNT protocol procedures (e.g., `Mnt`, `Umnt`, `Export`). This module aggregates them to form the complete `Mount` service interface.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the shared domain types (hostnames, mount records, export records) used across the MOUNT protocol implementation.
- To provide a unified service interface (`Mount` trait) that aggregates all specific MOUNT procedure traits, allowing the server to treat the MOUNT protocol as a single, cohesive entity.
- To enforce protocol-level constraints (specifically hostname length) at the type level via the `HostName` wrapper.

Inputs:
- **`name: String`**: Input for `HostName::new`, representing a client's hostname.

Outputs:
- **`HostName`**: A validated wrapper for the hostname string.
- **`MountEntry`**: A structure representing a client-to-directory mount mapping.
- **`ExportEntry`**: A structure representing a directory export and its allowed clients.
- **`MountRes`**: An enumeration wrapping the result types of various MOUNT procedures.
- **`Mount` Trait**: A super-trait combining `mnt::Mnt`, `umnt::Umnt`, `umntall::Umntall`, `export::Export`, and `dump::Dump`.

Steps:
1. **Hostname Validation**: When `HostName::new` is called, it checks if the length of the input string exceeds `MOUNT_HOST_NAME_LEN`. If it does, it returns `Err(io::Error::new(io::ErrorKind::InvalidInput, ...))`. Otherwise, it wraps the string in the `HostName` struct.
2. **Data Structure Definition**: The module defines `MountEntry` (containing `HostName` and `file::Path`) and `ExportEntry` (containing `file::Path` and a `Vec<HostName>`) to standardize how mount state and export configuration are represented.
3. **Trait Composition**: The `Mount` trait is defined with super-trait bounds requiring the implementation of `mnt::Mnt`, `umnt::Umnt`, `umntall::Umntall`, `export::Export`, and `dump::Dump`.
4. **Blanket Implementation**: A generic implementation `impl<T> Mount for T` is provided where `T` satisfies all the required sub-traits. This allows any type implementing the specific procedures to automatically satisfy the `Mount` contract.

Edge Cases:
- **`HostName` Length**: If the input string for `HostName` is longer than `MOUNT_HOST_NAME_LEN`, the constructor fails immediately.
- **`MountRes` Usage**: The `MountRes` enum is marked with `#[allow(dead_code)]`, indicating it might currently be unused or reserved for future use in a generic dispatcher that handles all MOUNT procedure results via a single type.

Complexity:
- **Time**: O(N) for `HostName::new`, where N is the length of the input string (due to the length check). O(1) for all other definitions.
- **Space**: O(N) for `HostName` and `MountEntry`/`ExportEntry` fields, proportional to the length of the strings/paths they contain.

Determinism:
- **Deterministic**: The validation logic and type definitions are deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
 - **`Path`**: This module relies on `file::Path` to store directory paths in `MountEntry` and `ExportEntry`. By using `file::Path`, the module delegates the validation of path constraints (e.g., maximum length, non-emptiness) to the VFS layer, ensuring consistency across the system.

- **From `nfs_mamont::consts::mount`**:
 - **`MOUNT_HOST_NAME_LEN`**: This constant is critical for the `HostName::new` constructor. It defines the hard limit for hostname size, ensuring that the MOUNT protocol implementation does not accept hostnames that violate the specification or cause buffer overflows in the RPC layer.

- **From `nfs_mamont::mount::<submodules>` (e.g., `mnt`, `export`, `dump`, `umnt`, `umntall`)**:
 - **Procedure Traits**: The `Mount` trait defined in this module acts as a facade, aggregating the specific traits (`Mnt`, `Export`, etc.) defined in the sub-modules. This allows the RPC layer or server logic to depend on a single `Mount` bound rather than managing each procedure trait separately.

---

## 4. Data Model

Entities:
- **`HostName`**: A wrapper for a client hostname string.
 - Invariants: The inner string length is `<= MOUNT_HOST_NAME_LEN`.
- **`MountEntry`**: Represents an active mount.
 - Fields: `hostname` (`HostName`), `directory` (`file::Path`).
- **`ExportEntry`**: Represents an exported filesystem.
 - Fields: `directory` (`file::Path`), `names` (`Vec<HostName>`).
- **`MountRes`**: An enum wrapping the results of MOUNT procedures.
 - Variants: `Null`, `Mount(Result<mnt::Success, mnt::Fail>)`, `Unmount`, `Export(export::Success)`, `Dump(dump::Success)`, `UnmountAll`.
- **`Mount`**: A trait combining all MOUNT procedure traits.

Relations:
- **Composition**: `MountEntry` and `ExportEntry` compose `HostName` and `file::Path`.
- **Aggregation**: The `Mount` trait aggregates the traits from the `mnt`, `umnt`, `umntall`, `export`, and `dump` modules.

Global Invariants:
- **Hostname Length**: All instances of `HostName` are guaranteed to contain strings with a length less than or equal to `MOUNT_HOST_NAME_LEN`.
- **Path Validity**: All instances of `file::Path` used within `MountEntry` and `ExportEntry` are guaranteed to be valid according to the invariants of the `vfs::file` module.

## 5. Error Model

Error Types:
- **`std::io::Error`**: The only error type produced by this module, specifically within the `HostName::new` constructor.

Error Propagation Strategy:
- **Constructor Validation**: The module uses the "Fallible Constructor" pattern. `HostName::new` returns `io::Result<Self>`. If the hostname is too long, it returns `Err(io::Error::new(io::ErrorKind::InvalidInput, "host name too long"))`.

Recoverability:
- **Recoverable**: Callers can handle the `Err` result, typically by rejecting the MOUNT request or returning a protocol-specific error (e.g., `mnt::Fail::NameTooLong`).

Panics:
- **Allowed**: No.
- **Conditions**: The module performs explicit checks and returns `Result` types rather than panicking on invalid input.

---

## 6. Traits

List which external traits this module implements:
- **`std::clone::Clone`**: Implemented for `HostName`, `MountEntry`, `ExportEntry`.
- **`std::fmt::Debug`**: Implemented for `HostName`, `MountEntry`, `ExportEntry`.
- **`std::cmp::PartialEq`**: Implemented for `HostName`, `MountEntry`, `ExportEntry`.
- **`std::hash::Hash`**: Implemented for `HostName`, `MountEntry`.
- **`std::cmp::Eq`**: Implemented for `HostName`, `MountEntry`.

List which traits this module defines:
- **`Mount`**: A super-trait that requires the implementation of `mnt::Mnt`, `umnt::Umnt`, `umntall::Umntall`, `export::Export`, and `dump::Dump`.

---

## 7. Overview

This module is used in order to **unify the data models and service interfaces for the NFS MOUNT protocol**. The system contains a modular implementation of the MOUNT protocol where specific procedures (like mounting, unmounting, dumping the mount table, and listing exports) are separated into their own sub-modules. However, these procedures share common concepts: they all deal with client hostnames and server directory paths. Without this module, these shared types would either be duplicated across sub-modules (leading to inconsistency) or defined in an unrelated location (leading to poor cohesion).

A typical usage scenario of the system involves the main server logic needing to store the state of active mounts. It uses the `MountEntry` struct defined here to represent a single mount record, associating a validated `HostName` with a `file::Path`. When the server needs to advertise which directories are available for mounting, it uses the `ExportEntry` struct. Furthermore, the RPC layer needs a single object to dispatch MOUNT requests to. This module provides the `Mount` trait, which aggregates all the individual procedure traits (`Mnt`, `Umnt`, etc.), allowing the server to hold a `dyn Mount` reference and invoke any procedure on it.

Inside the system, the following things happen and they use this module:
1. **Type Safety and Validation**: The `HostName` struct ensures that any hostname processed by the MOUNT protocol is validated against the `MOUNT_HOST_NAME_LEN` constant immediately upon creation. This prevents invalid data from propagating deeper into the system.
2. **State Representation**: The `MountEntry` and `ExportEntry` structs provide the canonical data structures for the server's internal mount table and export list. This ensures that whether the server is processing a MNT request or responding to a DUMP request, it uses the same format for data.
3. **Interface Aggregation**: The `Mount` trait allows the server to treat the MOUNT service as a single component. This is crucial for dependency injection, as the server can require a generic `M: Mount` bound without needing to know about the specific implementation details of each sub-procedure.

**Uncertainty**: The `MountRes` enum is defined but marked `#[allow(dead_code)]`. It appears to be intended as a generic wrapper for the results of all MOUNT procedures, potentially for use in a dynamic dispatcher or a unified RPC handler. However, since it is marked as dead code, it is possible that the current implementation uses the specific result types from the sub-modules directly, and `MountRes` is either legacy code or prepared for a future refactoring.