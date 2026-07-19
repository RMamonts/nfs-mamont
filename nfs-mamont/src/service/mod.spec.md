<!-- SPEC_HASH: 3a06b0f09d68076883488df838375f6d55cf3707814b64a126b7c15a2b6f69eb -->
# Module Specification

Module: nfs_mamont::service
Rust File: src/service/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::service::mount`**:
    *   **Purpose**: This submodule is declared here to encapsulate the server-side implementation of the MOUNT v3 protocol. It exposes `MountService`, which manages the state of exported directories and active client mounts.
*   **`crate::service::nlm`**:
    *   **Purpose**: This submodule is declared here to encapsulate the server-side implementation of the Network Lock Manager (NLM) v4 protocol. It exposes `NlmService`, which manages the state of file locks and pending lock requests.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a unified namespace for the service layer of the NFS server, grouping the concrete implementations of different RPC protocols (MOUNT and NLM) separately from their protocol definitions or the Virtual File System (VFS) layer.

Inputs:
- None (This is a module declaration file).

Outputs:
- **Public Module `mount`**: Exposes the MOUNT v3 service logic.
- **Public Module `nlm`**: Exposes the NLM v4 service logic.

Steps:
1.  **Namespace Declaration**: The module declares `pub mod mount;`, making the `mount` submodule (defined in `service/mount/mod.rs`) accessible to external crates as `nfs_mamont::service::mount`.
2.  **Namespace Declaration**: The module declares `pub mod nlm;`, making the `nlm` submodule (defined in `service/nlm/mod.rs`) accessible to external crates as `nfs_mamont::service::nlm`.

Edge Cases:
- None. This module performs no runtime logic.

Complexity:
- **Time**: N/A (Compile-time organization).
- **Space**: N/A.

Determinism:
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::service::mount`**:
    - **`MountService`**: This is the primary entity exposed through this module. It provides the centralized state management (exports and active mounts) required for the MOUNT protocol.
    - **`ExportEntryWrapper`**: A data structure used to initialize the `MountService`, representing the configuration of exported directories.

- **From `nfs_mamont::service::nlm`**:
    - **`NlmService`**: This is the primary entity exposed through this module. It provides the in-memory registry for managing file locks (active and pending) required for the NLM protocol.
    - **`LockRegistry`**: The internal state container within `NlmService` that handles conflict detection and lock queueing.

---

## 4. Data Model

Entities:
- None defined in this module.

Relations:
- None defined in this module.

Global Invariants:
- None.

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
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **structurally organize the service layer of the NFS server**, providing a clear separation between the abstract protocol definitions (likely found in `crate::mount` or `crate::nlm`) and their concrete, stateful implementations.

The system contains a complex NFS server implementation that must handle multiple distinct protocols simultaneously: MOUNT (for establishing initial file handles and authentication) and NLM (for advisory file locking). This module is necessary because it acts as the root of the "service" tree, ensuring that the specific logic for these protocols—such as how mount tables are stored in memory or how lock ranges are calculated—is encapsulated within its own submodules. This prevents the main application logic from being cluttered with implementation details and allows for a clean dependency graph where the main server loop can instantiate `nfs_mamont::service::mount::MountService` and `nfs_mamont::service::nlm::NlmService` independently.

A typical usage scenario of the system involves the main application initialization code. The application needs to start the RPC servers for MOUNT and NLM. To do this, it imports `MountService` from `nfs_mamont::service::mount` and `NlmService` from `nfs_mamont::service::nlm`. It configures the `MountService` with a list of exports and creates a default `NlmService`. These service instances are then registered with the RPC dispatcher. The `nfs_mamont::service` module facilitates this by providing a predictable path to these implementations.

Inside the system, the following things happen and they use this module:
1.  **Architectural Boundaries**: The module enforces the boundary where "protocol traits" end and "service logic" begins. The submodules (`mount`, `nlm`) contain the structs that hold the runtime state (`Arc<RwLock<...>>`), while this module simply exposes them.
2.  **Dependency Resolution**: When other parts of the application (e.g., the RPC server setup in `lib.rs`) need to reference these services, they do so via the path established by this module.

Without this module, the service implementations would likely reside at the top level of the crate or alongside their protocol definitions, potentially leading to circular dependencies or a cluttered namespace. This module ensures that the "how" (service implementation) is distinct from the "what" (protocol definition).

**Uncertainty**: The source code lists only `mount` and `nlm`. It is possible that other services (like `nfs` for the core NFS protocol) exist in the full project but were not included in the provided context. This specification covers only the explicitly declared submodules.