<!-- SPEC_HASH: 97775eb17e4fca401de6f66eaf0ad43ec01f4ba70407d13d53c3759000469db7 -->
# Module Specification

Module: nfs_mamont::consts
Rust File: src/consts/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **nfs_mamont::consts::mount**: This submodule is used to encapsulate all constant definitions related to the MOUNT protocol (Version 3), including RPC program numbers, procedure identifiers, and path size limits. It is included to namespace these specific constants away from the main NFS protocol logic.
- **nfs_mamont::consts::nfsv3**: This submodule is used to encapsulate all constant definitions related to the NFS Version 3 protocol, including program numbers, procedure IDs, and file handle sizes. It is included to provide the core numerical identifiers required for NFSv3 communication.
- **nfs_mamont::consts::nlm**: This submodule is used to encapsulate all constant definitions related to the Network Lock Manager (NLM) Version 4 protocol. It is included to namespace the identifiers required for file locking operations.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To act as a namespace aggregator for the various protocol constant modules within the `nfs_mamont` crate. It provides a single entry point (`nfs_mamont::consts`) for accessing the static configuration of all supported NFS-related protocols (MOUNT, NFSv3, NLM).

Inputs:
- None. This module consists solely of module declarations.

Outputs:
- Public visibility of the submodules `mount`, `nfsv3`, and `nlm`.

Steps:
1. The module declares `pub mod mount;`, making the `mount` submodule (containing MOUNT protocol constants) publicly accessible under the `nfs_mamont::consts::mount` path.
2. The module declares `pub mod nfsv3;`, making the `nfsv3` submodule (containing NFSv3 protocol constants) publicly accessible under the `nfs_mamont::consts::nfsv3` path.
3. The module declares `pub mod nlm;`, making the `nlm` submodule (containing NLM protocol constants) publicly accessible under the `nfs_mamont::consts::nlm` path.

Edge Cases:
- None. This is a structural module with no runtime logic.

Complexity:
- Time: O(1) (Compile-time module resolution).
- Space: O(1) (No runtime data allocation).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::consts::mount`**: Provides the static registry for MOUNT protocol identifiers (program number `100005`, version `3`, procedure IDs) and size constraints (`MOUNT_DIRPATH_LEN`, `MOUNT_HOST_NAME_LEN`).
- **From `nfs_mamont::consts::nfsv3`**: Provides the static registry for NFSv3 protocol identifiers (program number `100003`, version `3`, procedure IDs) and data structure sizes (`NFS3_FHSIZE`).
- **From `nfs_mamont::consts::nlm`**: Provides the static registry for NLM protocol identifiers (program number `100021`, version `4`, procedure IDs) and buffer size limits (`LM_MAXSTRLEN`, `OPAQUE_HANDLE_SIZE`).

---

## 4. Data Model

Entities:
- **Submodules**: `mount`, `nfsv3`, `nlm`. These are the only entities defined at this level, acting as containers for constant data.

Relations:
- **Aggregation**: The `consts` module aggregates the three protocol-specific submodules.

Global Invariants:
- The module structure enforces a strict separation of concerns, ensuring that constants for distinct protocols (MOUNT, NFS, NLM) do not pollute each other's namespaces.

## 5. Error Model

Error Types:
- None.

Error Propagation Strategy:
- N/A.

Recoverability:
- N/A.

Panics:
- Allowed: No
- Conditions: This module contains only module declarations; no executable code exists that could result in a panic.

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to organize the "magic numbers" required for the `nfs_mamont` NFS stack into a logical hierarchy. The system contains a complex implementation of an NFS server (and potentially client) that must interoperate with standard clients using three distinct but related protocols: the MOUNT protocol (for initial filesystem access), the NFSv3 protocol (for file operations), and the NLM protocol (for file locking). A typical usage scenario of the system involves an RPC dispatcher that needs to identify an incoming network packet. The dispatcher imports constants from this module's submodules (e.g., `nfs_mamont::consts::nfsv3::NFS3_PROGRAM`) to match the packet's program number against the supported services. Inside the system, the following things happen and they use this module to prevent identifier collisions; for example, both MOUNT and NFS have procedures with similar names (like "NULL"), and separating them into `consts::mount` and `consts::nfsv3` namespaces allows the code to refer to `mount::MOUNT_NULL` and `nfsv3::NFSPROC3_NULL` unambiguously. Without this aggregation module, the project structure would lack a clear root for protocol configuration, leading to scattered definitions and potential maintenance issues when updating protocol versions.