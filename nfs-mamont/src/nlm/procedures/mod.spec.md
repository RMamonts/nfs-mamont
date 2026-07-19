<!-- SPEC_HASH: 6f20aeadb004a59f6c8838c1e6a44ca56af2315b572222024fd029ac167b3604 -->
# Module Specification

Module: nfs_mamont::nlm::procedures
Rust File: src/nlm/procedures/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::procedures::lock`**:
    *   Used to provide the implementation and data structures for the `NLM4_LOCK` procedure (procedure number 2). It exposes the `Lock` trait, `Nlm4LockArgs`, and `Nlm4LockRes`.
*   **`crate::nlm::procedures::test`**:
    *   Used to provide the implementation and data structures for the `NLM4_TEST` procedure (procedure number 1). It exposes the `Test` trait, `Nlm4TestArgs`, and `Nlm4TestRes`.
*   **`crate::nlm::procedures::unlock`**:
    *   Used to provide the implementation and data structures for the `NLM4_UNLOCK` procedure (procedure number 4). It exposes the `Unlock` trait, `Nlm4UnlockArgs`, and `Nlm4UnlockRes`.
*   **`crate::nlm::procedures::cancel`**:
    *   Used to provide the implementation and data structures for the `NLM4_CANCEL` procedure (procedure number 3). It exposes the `Cancel` trait, `Nlm4CancelArgs`, and `Nlm4CancelRes`.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the canonical mapping between NLMv4 RPC procedure numbers (integers) and their semantic identifiers. This serves as the central registry for the RPC dispatcher to route incoming requests to the correct handler logic defined in the submodules.

Inputs:
- None (This module defines static data and module organization).

Outputs:
- **`Nlm4Procedures`**: A public enumeration where each variant corresponds to a specific NLMv4 procedure number defined in RFC 1813.

Steps:
1.  The module is initialized by the Rust runtime.
2.  The `Nlm4Procedures` enum is compiled into the binary, providing constant values (e.g., `Lock = 2`) that the RPC layer can use for pattern matching or lookup tables.

Edge Cases:
- **Procedure 0 (Null)**: The enum includes `Null = 0`, which corresponds to the standard RPC null procedure used for ping/liveness checks. While the other procedures map to specific logic in submodules, `Null` is handled implicitly by the RPC framework or requires no specific logic in this module other than its definition.

Complexity:
- **Time**: O(1) (Accessing enum variants is a compile-time constant).
- **Space**: O(1) (The enum type itself occupies no runtime memory beyond its usage).

Determinism:
- **Deterministic**. The mapping of integers to procedure names is fixed by the NLMv4 protocol specification.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

-   **From `crate::nlm::procedures::lock`**:
    -   **`Lock` Trait**: Defines the asynchronous interface for handling lock acquisition. This module relies on this trait to provide the actual implementation for the `Lock` procedure number.
    -   **`Nlm4LockArgs` / `Nlm4LockRes`**: Define the wire format for the arguments and results of the Lock procedure.
-   **From `crate::nlm::procedures::test`**:
    -   **`Test` Trait**: Defines the asynchronous interface for polling lock status. This module relies on this trait to provide the implementation for the `Test` procedure number.
    -   **`Nlm4TestArgs` / `Nlm4TestRes`**: Define the wire format for the arguments and results of the Test procedure.
-   **From `crate::nlm::procedures::unlock`**:
    -   **`Unlock` Trait**: Defines the asynchronous interface for releasing locks. This module relies on this trait to provide the implementation for the `Unlock` procedure number.
    -   **`Nlm4UnlockArgs` / `Nlm4UnlockRes`**: Define the wire format for the arguments and results of the Unlock procedure.
-   **From `crate::nlm::procedures::cancel`**:
    -   **`Cancel` Trait**: Defines the asynchronous interface for canceling pending lock requests. This module relies on this trait to provide the implementation for the `Cancel` procedure number.
    -   **`Nlm4CancelArgs` / `Nlm4CancelRes`**: Define the wire format for the arguments and results of the Cancel procedure.

---

## 4. Data Model

Entities:
- **`Nlm4Procedures`**: An enumeration representing the valid procedure numbers for the NLMv4 protocol.
    - `Null`: Value 0.
    - `Test`: Value 1.
    - `Lock`: Value 2.
    - `Cancel`: Value 3.
    - `Unlock`: Value 4.

Relations:
- **Aggregation**: This module aggregates the submodules (`lock`, `test`, `unlock`, `cancel`), effectively grouping their respective data models (`Args` and `Res` structs) and traits under the `nfs_mamont::nlm::procedures` namespace.

Global Invariants:
- The integer values assigned to `Nlm4Procedures` variants must strictly match the procedure numbers defined in RFC 1813 (NLM version 4) to ensure interoperability with standard NFS clients.

## 5. Error Model

Error Types:
- None defined in this module.

Error Propagation Strategy:
- N/A (This module does not contain executable logic that propagates errors).

Recoverability:
- N/A.

Panics:
- Allowed: No.
- Conditions: The module consists solely of an enum definition and module declarations. There is no runtime code capable of panicking.

---

## 6. Traits

List which external traits this module implements:
- None.

Traits defined by this module:
- None.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to provide a structured, protocol-compliant entry point for the Network Lock Manager version 4 (NLMv4) service within the NFS server implementation. The NLM protocol requires a specific set of operations (Lock, Unlock, Test, Cancel, Null) identified by unique integer procedure numbers. This module serves as the central registry that maps these integers to logical operations and organizes the implementation details of those operations into distinct, manageable submodules.

The system containing this module is an NLMv4 server that handles file locking requests from NFS clients. It relies on this module to define the "routing" logic (via `Nlm4Procedures`) and to namespace the specific request/response types and handler traits for each procedure. Without this module, the RPC layer would lack a standardized mapping between incoming packet headers and the specific Rust traits (e.g., `Lock`, `Test`) required to process them, leading to a disorganized and potentially non-compliant implementation.

A typical usage scenario of the system involves the RPC transport layer receiving an NLM request packet. The layer extracts the procedure number from the packet header. It uses the `Nlm4Procedures` enum (or its underlying values) to determine which handler to invoke. For example, if the procedure number is 2, the system identifies this as `Nlm4Procedures::Lock`. The system then delegates the processing to the logic defined in the `lock` submodule, specifically invoking an implementation of the `Lock` trait with the deserialized `Nlm4LockArgs`.

Inside the system, the following things happen and they use this module:
1.  **Dispatching**: The main RPC dispatcher uses the constants defined in `Nlm4Procedures` to match against the request header and select the correct service handler.
2.  **Type Aggregation**: The parent `nlm` module re-exports the contents of this module. This allows the rest of the application to access procedure-specific types (like `Nlm4LockArgs`) and traits (like `Lock`) through a single, hierarchical path (`nfs_mamont::nlm::procedures::...`), keeping the global namespace clean.
3.  **Protocol Compliance**: By explicitly defining the enum values according to RFC 1813, this module ensures that the server speaks the same language as standard NFS clients, preventing communication errors caused by mismatched procedure numbers.