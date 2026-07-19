<!-- SPEC_HASH: bc5abdd30064167ed54e65ce5d93198f85519d72b35c913ece5fa3ffc91bed82 -->
# Module Specification

Module: nfs_mamont::mount::export
Rust File: src/mount/export.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`super::ExportEntry`**: Used as the item type within the `Success` struct. It provides the structure for individual export records, linking a directory path to a list of allowed client hostnames.
- **`trait_variant::make`**: Used as a procedural macro attribute on the `Export` trait. It transforms the trait definition to ensure that the resulting trait object is `Send`, allowing the implementation to be safely passed across thread boundaries in an asynchronous context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the asynchronous interface for the MOUNT protocol version 3 "EXPORT" procedure (Procedure 5), as specified in RFC 1813.
- To provide a structured return type (`Success`) that encapsulates the list of exported filesystems and their access control lists.

Inputs:
- `&self`: A reference to the implementor of the trait.

Outputs:
- `Success`: A struct containing a vector of `ExportEntry` items.

Steps:
1. **Trait Definition**: The `Export` trait is defined with the `#[trait_variant::make(Send)]` attribute. This ensures that any dynamic reference (`dyn Export`) is thread-safe.
2. **Method Declaration**: The `export` method is declared as an asynchronous function (`async fn`).
3. **Result Struct Definition**: The `Success` struct is defined to wrap the `Vec<ExportEntry>`, serving as the payload for the successful response.

Edge Cases:
- **Protocol Errors**: The code explicitly notes that there are no MOUNT protocol errors defined for this procedure in RFC 1813. Consequently, the method returns `Success` directly rather than a `Result` type.

Complexity:
- **Time**: Not defined in this module (interface only).
- **Space**: O(N) where N is the number of exported directories, determined by the size of the `Vec<ExportEntry>`.

Determinism:
- Deterministic (Interface definition).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::mount`**:
 - **Path Validation**: The `Success` struct relies on `ExportEntry`, which in turn uses `file::Path`. This ensures that the directory paths returned by the `export` procedure are validated and conform to VFS constraints.
 - **Protocol Limits**: The `ExportEntry` struct uses `HostName`, which enforces the `MOUNT_HOST_NAME_LEN` limit. This ensures that the client names included in the export list are protocol-compliant.
 - **Procedure Interfaces**: The `Export` trait defined in this module is intended to be composed into the `Mount` super-trait defined in the parent module, allowing the server to handle this specific RPC as part of the unified MOUNT service.

---

## 4. Data Model

Entities:
- **`Success`**: A structure representing the successful result of the EXPORT procedure. It contains a list of all exported file systems.

Relations:
- **`Success` → `Vec<ExportEntry>` (Composition)**: The `Success` struct owns a vector of export entries.

Global Invariants:
- The `exports` vector within `Success` must contain valid `ExportEntry` instances, implying that all directory paths and hostnames within those entries adhere to the invariants defined in the parent module (e.g., valid paths, hostname length limits).

## 5. Error Model

Error Types:
- None. The interface does not define a failure return type.

Error Propagation Strategy:
- N/A. The method signature is `async fn export(&self) -> Success`, indicating that the procedure is expected to always succeed or that any internal errors are handled/hidden by the implementation (as per RFC 1813).

Recoverability:
- N/A.

Panics:
- **Allowed**: Unknown (depends on implementation).
- **Conditions**: The interface itself does not enforce panic conditions, but implementations may panic if internal invariants (e.g., VFS access) are violated.

---

## 6. Traits

List which external traits this module implements:
- **`Export`**: A trait defined in this module. It requires the implementation of an asynchronous `export` method that returns a `Success` struct. The trait is marked `Send` via the `trait_variant` macro.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **define the interface for retrieving the server's export list**, which is a specific capability within the broader NFS MOUNT protocol implementation. The system contains a unified MOUNT service that handles various client requests, such as mounting specific directories or unmounting them. However, clients also need a way to discover which directories are actually available for mounting and which specific clients are authorized to access them.

A typical usage scenario of the system involves an NFS client sending an `EXPORT` RPC request to the server. The RPC dispatcher, holding a reference to the unified `Mount` service (which aggregates the `Export` trait defined here), invokes the `export` method. The system then uses this module's `Success` struct to package the list of `ExportEntry` records. These records, defined in the parent module, utilize validated `file::Path` and `HostName` types to ensure the response is both type-safe and protocol-compliant.

Inside the system, the following things happen and they use this module:
1. **Service Discovery**: The `Export` trait provides the contract that allows the server to expose its filesystem topology to clients. This is critical for clients to understand the namespace before attempting to mount specific paths.
2. **Type Consistency**: By using `super::ExportEntry` in its return type, this module ensures that the data returned by the EXPORT procedure is structurally identical to the data used internally for access control decisions, maintaining consistency across the MOUNT subsystem.
3. **Thread Safety**: The use of the `trait_variant::make(Send)` macro ensures that the implementation of this procedure can be executed in an asynchronous, multi-threaded environment, which is essential for the high-performance requirements of the NFS server.

Without this module, the MOUNT protocol implementation would lack a standardized way to represent the EXPORT procedure's result, making it impossible for clients to programmatically discover available resources.