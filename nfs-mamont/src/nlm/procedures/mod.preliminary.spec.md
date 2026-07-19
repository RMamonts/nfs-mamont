<!-- SPEC_HASH: 6f20aeadb004a59f6c8838c1e6a44ca56af2315b572222024fd029ac167b3604 -->
# Module Specification

Module: nfs_mamont::nlm::procedures
Rust File: src/nlm/procedures/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

*   **`crate::nlm::procedures::lock`**:
    *   Used to encapsulate the logic, data structures (`Nlm4LockArgs`, `Nlm4LockRes`), and the asynchronous trait (`Lock`) for the NLMv4 `LOCK` procedure (procedure number 2). This module is re-exported to provide the implementation for acquiring file locks.
*   **`crate::nlm::procedures::test`**:
    *   Used to encapsulate the logic, data structures (`Nlm4TestArgs`, `Nlm4TestRes`), and the asynchronous trait (`Test`) for the NLMv4 `TEST` procedure (procedure number 1). This module is re-exported to provide the implementation for polling lock status without acquiring it.
*   **`crate::nlm::procedures::unlock`**:
    *   Used to encapsulate the logic, data structures (`Nlm4UnlockArgs`, `Nlm4UnlockRes`), and the asynchronous trait (`Unlock`) for the NLMv4 `UNLOCK` procedure (procedure number 4). This module is re-exported to provide the implementation for releasing file locks.
*   **`crate::nlm::procedures::cancel`**:
    *   Used to encapsulate the logic, data structures (`Nlm4CancelArgs`, `Nlm4CancelRes`), and the asynchronous trait (`Cancel`) for the NLMv4 `CANCEL` procedure (procedure number 3). This module is re-exported to provide the implementation for canceling pending blocked lock requests.

*Note: The specific `*.deps.json` file for this module was not provided in the context. Dependencies listed above are inferred from the `pub mod` declarations in the source code and the provided specifications of the sub-modules.*

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module.
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the canonical mapping between NLMv4 RPC procedure numbers (integers) and their semantic meanings within the `nfs_mamont` crate. Additionally, to aggregate and expose the specific procedure implementations (lock, test, unlock, cancel) under a unified namespace.

Inputs:
- None (This is a declaration and organization module).

Outputs:
- `Nlm4Procedures`: A public enumeration where each variant corresponds to a specific NLMv4 RPC procedure number defined in RFC 1813.
- Public sub-modules: `lock`, `test`, `unlock`, `cancel`.

Steps:
1.  **Enum Definition**: The module defines `Nlm4Procedures` with explicit discriminants (e.g., `Lock = 2`) to strictly adhere to the protocol constants.
2.  **Module Aggregation**: The module declares and re-exports sub-modules (`lock`, `test`, `unlock`, `cancel`). This organizes the codebase by separating the protocol definition (this file) from the procedure-specific logic and data structures (sub-modules).

Edge Cases:
- None. The module consists of static declarations.

Complexity:
- Time: O(1) (Compile-time resolution).
- Space: O(1) (Size of the enum).

Determinism:
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

-   **From `nfs_mamont::nlm::procedures::lock`**:
    -   **`Lock` Trait**: Defines the asynchronous contract for handling lock requests.
    -   **`Nlm4LockArgs` / `Nlm4LockRes`**: Data Transfer Objects (DTOs) for the lock operation.
-   **From `nfs_mamont::nlm::procedures::test`**:
    -   **`Test` Trait**: Defines the asynchronous contract for polling lock conflicts.
    -   **`Nlm4TestArgs` / `Nlm4TestRes`**: DTOs for the test operation.
-   **From `nfs_mamont::nlm::procedures::unlock`**:
    -   **`Unlock` Trait**: Defines the asynchronous contract for releasing locks.
    -   **`Nlm4UnlockArgs` / `Nlm4UnlockRes`**: DTOs for the unlock operation.
-   **From `nfs_mamont::nlm::procedures::cancel`**:
    -   **`Cancel` Trait**: Defines the asynchronous contract for canceling blocked requests.
    -   **`Nlm4CancelArgs` / `Nlm4CancelRes`**: DTOs for the cancel operation.

---

## 4. Data Model

Entities:
-   **`Nlm4Procedures`**: An enumeration representing the NLMv4 RPC procedure numbers.
    -   `Null` (0): No operation.
    -   `Test` (1): Test for a lock.
    -   `Lock` (2): Request a lock.
    -   `Cancel` (3): Cancel an outstanding lock request.
    -   `Unlock` (4): Release a lock.

Relations:
-   None. The enum variants are mutually exclusive and independent scalar values.

Global Invariants:
-   The discriminant values of `Nlm4Procedures` variants must match the procedure numbers specified in RFC 1813 for NLMv4.

---

## 5. Error Model

Error Types:
- None defined in this module.

Error Propagation Strategy:
- N/A.

Recoverability:
- N/A.

Panics:
- Allowed: No.
- Conditions: The module contains only type definitions and module declarations, which cannot panic.

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

This module is used in order to provide a centralized, protocol-compliant entry point for the Network Lock Manager (NLM) version 4 implementation within the `nfs_mamont` project. It serves as the bridge between the generic RPC dispatch layer—which operates on raw procedure numbers—and the specific business logic required to handle locking operations.

This system contains the `Nlm4Procedures` enum, which acts as the authoritative source of truth for mapping integer procedure IDs (received over the network) to semantic operations (Lock, Unlock, etc.). By aggregating the `lock`, `test`, `unlock`, and `cancel` sub-modules, it creates a cohesive namespace that groups all NLMv4-related functionality. This separation allows the RPC layer to remain agnostic to the details of lock management; it simply matches the incoming procedure number to a variant in `Nlm4Procedures` and delegates the execution to the corresponding trait implementation found in the sub-modules.

A typical usage scenario of the system involves an NFS server receiving an RPC request. The RPC dispatcher inspects the procedure number. If it is `2`, it identifies the operation as `Nlm4Procedures::Lock`. The server logic then invokes the handler provided by the `lock` submodule (specifically, an implementation of the `Lock` trait). This handler uses the `Nlm4LockArgs` to interpret the request payload and returns an `Nlm4LockRes`. The `procedures` module ensures that all these components are accessible and logically grouped, enforcing the structure defined by the NLM protocol standard.

Inside the system the following things happen and they use the `Nlm4Procedures` enum to maintain strict type safety regarding protocol constants, preventing "magic numbers" from being scattered throughout the codebase. The sub-modules handle the complexity of asynchronous state management, conflict resolution, and data serialization, while this parent module ensures they are presented as a unified interface to the rest of the application.