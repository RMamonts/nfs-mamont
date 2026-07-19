<!-- SPEC_HASH: eefbc4169d4d0ae441cbc9a0e70e66fe287cb3026ec41ca76642d6e82da04fa0 -->
# Module Specification

Module: nfs_mamont::nlm::procedures::test
Rust File: src/nlm/procedures/test.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::nlm::cookie::Cookie`**: Used to provide a transaction identifier in both the request (`Nlm4TestArgs`) and response (`Nlm4TestRes`). This allows the client to match asynchronous RPC replies to their original requests.
- **`crate::nlm::holder::Nlm4Holder`**: Used within the `Nlm4TestReply` structure to describe the owner of a conflicting lock when a test request is denied. It carries the system ID, opaque handle, and lock range of the blocker.
- **`crate::nlm::lock::Nlm4Lock`**: Used within `Nlm4TestArgs` to specify the parameters of the lock being tested (caller name, file handle, owner, offset, length).
- **`crate::nlm::Nlm4Stats`**: Used within `Nlm4TestReply` to indicate the result of the test operation (e.g., `Granted`, `Denied`, `DeniedNolocks`).
- **`trait_variant::make`**: Used as a procedural macro attribute on the `Test` trait. It transforms the async trait definition into a standard trait that is object-safe and implements `Send`, allowing the trait to be used as a trait object (e.g., `dyn Test`) in async contexts.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the data structures and the asynchronous service interface for the NLMv4 `TEST` procedure (RFC 1813). This procedure allows a client to query the server to determine if a specific lock request would be granted without actually acquiring the lock.

Inputs:
- **`Nlm4TestArgs`**: The input structure for the procedure.
  - `cookie`: `Cookie` - Transaction identifier.
  - `exclusive`: `bool` - Lock type (exclusive vs shared).
  - `lock`: `Nlm4Lock` - The lock details to test.
- **`Test` trait**: The interface implemented by the NLM service handler.
  - `args`: `Nlm4TestArgs` - The arguments passed to the handler.

Outputs:
- **`Nlm4TestRes`**: The result structure returned to the client.
  - `cookie`: `Cookie` - Echoed from the request.
  - `test_stat`: `Nlm4TestReply` - The status and optional holder information.

Steps:
1. **Request Construction**: The client (or RPC layer) constructs `Nlm4TestArgs` with the desired lock parameters.
2. **Dispatch**: The `test` method is invoked on an object implementing the `Test` trait.
3. **Evaluation**: The implementation checks the server's internal lock state against the provided `Nlm4Lock` and `exclusive` flag.
4. **Response Formulation**:
   - If the lock can be granted, the implementation returns `Nlm4TestRes` with `test_stat.stat` set to `Nlm4Stats::Granted` and `test_stat.holder` set to `None`.
   - If the lock cannot be granted (conflict exists), the implementation returns `Nlm4TestRes` with `test_stat.stat` set to `Nlm4Stats::Denied` (or another error code) and populates `test_stat.holder` with `Some(Nlm4Holder)` describing the conflicting lock.

Edge Cases:
- **Holder Availability**: The `holder` field in `Nlm4TestReply` is an `Option`. While the protocol suggests providing holder info on denial, the type system allows `None` even if the status is `Denied` (e.g., if the lock is denied due to resource limits rather than a specific conflict).
- **Zero Length Lock**: The `Nlm4Lock` contained within the arguments may have a length of 0, which semantically means "lock to end of file". This module does not interpret this; it passes the data through to the trait implementation.

Complexity:
- **Time**: O(1) for data structure access. The complexity of the actual lock check depends on the implementation of the `Test` trait.
- **Space**: O(1) for the structs themselves, plus the heap space required for the internal `String` in `Nlm4Lock` and `Vec<u8>` in `Nlm4Holder`/`Cookie` dependencies.

Determinism:
- Deterministic (regarding data structure definitions). The trait implementation's determinism depends on the specific service logic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::nlm::cookie`**:
  - **Transaction Identification**: The `Cookie` type is used to ensure that the `cookie` field in `Nlm4TestRes` matches the one in `Nlm4TestArgs`, maintaining request-response correlation in the RPC layer.
- **From `nfs_mamont::nlm::holder`**:
  - **Conflict Description**: The `Nlm4Holder` structure is used to populate the `holder` field in `Nlm4TestReply`. It provides the client with the identity (system ID, opaque handle) and range of the lock that is blocking the request.
- **From `nfs_mamont::nlm::lock`**:
  - **Request Validation**: The `Nlm4Lock` structure encapsulates the lock parameters. It relies on its own constructor to validate the `caller_name` length before the request reaches the `Test` handler.
- **From `nfs_mamont::nlm` (parent module)**:
  - **Status Codes**: The `Nlm4Stats` enum provides the vocabulary for the `stat` field in `Nlm4TestReply`, allowing the server to indicate success (`Granted`) or various failure modes (`Denied`, `DeniedNolocks`, etc.).

---

## 4. Data Model

Entities:
- **`Nlm4TestArgs`**: A structure representing the arguments for the NLM TEST procedure. It aggregates the transaction cookie, lock exclusivity flag, and the specific lock description.
- **`Nlm4TestRes`**: A structure representing the result of the NLM TEST procedure. It contains the echoed cookie and the test status union.
- **`Nlm4TestReply`**: A structure acting as a discriminated union for the test result. It holds a status code and optionally, the holder of a conflicting lock.
- **`Test`**: An asynchronous trait defining the contract for handling TEST requests.

Relations:
- **Composition**: `Nlm4TestArgs` contains `Cookie` and `Nlm4Lock`.
- **Composition**: `Nlm4TestRes` contains `Cookie` and `Nlm4TestReply`.
- **Composition**: `Nlm4TestReply` contains `Nlm4Stats` and `Option<Nlm4Holder>`.

Global Invariants:
- **Semantic Consistency**: If `Nlm4TestReply.stat` is `Granted`, the `holder` field should logically be `None`. If `stat` is `Denied`, `holder` should typically be `Some` to provide useful debugging information to the client, though this is not enforced by the type system.

## 5. Error Model

Error Types:
- None defined in this module. The `test` method returns `Nlm4TestRes` directly, not a `Result`.

Error Propagation Strategy:
- **Status Codes**: Errors are propagated via the `stat` field of `Nlm4TestReply` using the `Nlm4Stats` enum. For example, a failure to allocate resources would result in `stat` being `Nlm4Stats::DeniedNolocks`.

Recoverability:
- **Client-Side**: The client must inspect the `stat` field to determine if the lock is available. If `Denied`, the client may choose to retry later or notify the user.

Panics:
- Allowed: No.
- Conditions: The public interface consists solely of struct definitions and an async trait definition, neither of which can panic directly.

---

## 6. Traits

List which external traits this module implements:
- None.

Traits defined by this module:
- **`Test`**: An asynchronous trait (made `Send` via `trait_variant`) requiring the implementation of the `test` method. This trait is intended to be implemented by the NLM service handler to process `NLMPROC4_TEST` RPC calls.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to implement the non-blocking conflict detection mechanism of the Network Lock Manager (NLM) protocol. In distributed file systems, clients often need to check the availability of a lock before attempting an operation that might block, or they need to diagnose why a previous lock attempt failed. The `TEST` procedure provides this capability by allowing a client to query the server's lock state without modifying it.

The system containing this module is an NLMv4 server that manages file locks for NFS clients. It relies on a set of procedure modules (`lock`, `unlock`, `test`, `cancel`) to handle specific protocol operations. This module defines the `Test` trait and its associated data structures (`Nlm4TestArgs`, `Nlm4TestRes`), which serve as the contract for the `TEST` operation.

A typical usage scenario of the system involves a client attempting to access a file and receiving a "Denied" response to a lock request. To understand why, the client constructs a `Nlm4TestArgs` structure with the same lock parameters and invokes the `test` method via RPC. The server's implementation of the `Test` trait checks its internal lock table. If a conflict exists, it returns a `Nlm4TestRes` containing `Nlm4Stats::Denied` and a `Nlm4Holder` structure describing the process that holds the conflicting lock. This allows the client to report meaningful information to the user or application (e.g., "File is locked by host X, PID Y").

Inside the system, this module ensures that the data exchanged during a TEST operation is strongly typed and validated. By using `Nlm4Lock` (which validates caller names) and `Nlm4Holder` (which encapsulates owner identity), the module abstracts away the raw bytes of the NLM protocol, providing a clean interface for the higher-level RPC dispatcher and the core locking logic. The `Test` trait is aggregated into the main `Nlm` service trait (defined in `nlm/mod.rs`), ensuring that any valid NLM service implementation supports conflict polling.