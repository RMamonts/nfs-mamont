<!-- SPEC_HASH: b4ff4a83916877b54ed5e7209c21de645b8e1ec5ec65680595cb1e89d22f5d39 -->
# Module Specification

Module: nfs_mamont::service::mount::export
Rust File: src/service/mount/export.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::mount::export::{Export, Success}`**: Used to define the implementation of the MOUNT protocol's EXPORT procedure. The `Export` trait provides the interface, and `Success` is the return type struct that wraps the list of exported filesystems.
- **`super::MountService`**: The struct for which the `Export` trait is implemented. This allows the `MountService` to act as the handler for EXPORT RPC requests.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide the concrete implementation of the `Export` trait for the `MountService`.
- To retrieve the server's export list from the internal registry and format it according to the MOUNT v3 protocol specification.

Inputs:
- `&self`: A reference to the `MountService` instance.

Outputs:
- `Success`: A structure containing a vector of `ExportEntry` items representing the exported filesystems.

Steps:
1. **Access Registry**: The implementation accesses the `exports` field of `MountService`.
2. **Retrieve List**: It calls the `export_list()` method on the `exports` field.
   - *Assumption*: The `exports` field (type `Arc<ExportRegistry>` based on dependency context) possesses a method `export_list()` that returns a `Vec<ExportEntry>`. This method is not explicitly listed in the provided public interface facts for `MountService` but is required by the code in this module.
3. **Construct Response**: The retrieved vector is wrapped in the `Success` struct and returned.

Edge Cases:
- **Empty Exports**: If the server has no exports, `export_list()` is expected to return an empty vector, resulting in `Success { exports: vec![] }`.
- **Protocol Errors**: The interface does not allow returning errors (returns `Success` directly). Any issues retrieving the list (e.g., internal logic errors) would likely result in a panic or be handled internally by `export_list()`.

Complexity:
- **Time**: O(N), where N is the number of exported directories. This depends on the implementation of `export_list()`.
- **Space**: O(N), required to store the vector of `ExportEntry` in the `Success` struct.

Determinism:
- Deterministic (assuming `export_list()` is deterministic).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::mount::export`**:
 - **Interface Definition**: The `Export` trait defines the contract `async fn export(&self) -> Success`. This module fulfills this contract.
 - **Data Structure**: The `Success` struct is used to encapsulate the result. It relies on `ExportEntry` (defined in `nfs_mamont::mount`) to represent individual export records.

- **From `nfs_mamont::service::mount`**:
 - **State Container**: The `MountService` acts as the context. It holds the `exports` registry.
 - **Uncertainty**: The provided specification for `MountService` lists `exports` as `Arc<ExportRegistry>` but does not explicitly list an `export_list()` method in its public interface. However, the code in this module (`self.exports.export_list()`) implies that either `ExportRegistry` or `MountService` exposes this functionality to retrieve the full list of exports.

---

## 4. Data Model

Entities:
- This module defines no new entities. It utilizes `Success` and `ExportEntry` from dependencies.

Relations:
- **`MountService` implements `Export`**: The service struct provides the logic required by the protocol trait.

Global Invariants:
- The `exports` field within `MountService` must be initialized and accessible to return a valid list of exports.

## 5. Error Model

Error Types:
- None. The function signature `async fn export(&self) -> Success` does not include a `Result` type.

Error Propagation Strategy:
- N/A. The method is infallible in the signature. Any internal errors during the retrieval of the export list must be handled internally (e.g., panicking or returning an empty list) to satisfy the interface.

Recoverability:
- N/A.

Panics:
- **Allowed**: Unknown (depends on the implementation of `self.exports.export_list()`).
- **Conditions**: Potential panic if the `exports` registry is in an invalid state or if `export_list` encounters an unrecoverable error.

---

## 6. Traits

List which external traits this module implements:
- **`crate::mount::export::Export`**: Implemented for `MountService`. This allows the service to handle the MOUNT v3 EXPORT procedure.

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **realize the MOUNT v3 EXPORT procedure within the concrete service layer**, effectively bridging the abstract protocol requirements with the server's internal state management. The system contains a clear architectural separation: the `nfs_mamont::mount` module defines the *language* of the protocol (traits and data structures), while the `nfs_mamont::service::mount` module defines the *memory and state* of the server. This module is the critical link that translates a request for the "export list" into a specific query against the `MountService`'s internal registry.

A typical usage scenario of the system involves an NFS client sending an `EXPORT` RPC request to discover available filesystems. The RPC dispatcher, holding a reference to the `MountService` (likely cast as a `dyn Mount` or `dyn Export`), invokes the `export` method. This module's implementation executes, retrieving the static configuration of exported directories from the `exports` registry and packaging it into the `Success` structure defined by the protocol.

Inside the system, the following things happen and they use this module:
1.  **Service Composition**: The `MountService` aggregates various procedure implementations. By implementing the `Export` trait here, the `MountService` satisfies the bounds required by the `Mount` super-trait, allowing it to be registered as a complete handler for all MOUNT procedures.
2.  **Data Access**: The module delegates the complexity of storage to the `exports` field (assumed to be an `ExportRegistry`). It assumes the registry can provide a full list (`export_list()`), abstracting away whether the data is stored in a `HashMap`, a list, or fetched from a database.

Without this module, the `MountService` would be incomplete; it would be unable to respond to `EXPORT` requests, causing the RPC handler to fail or the service to fail to compile against the required trait bounds.