<!-- SPEC_HASH: 9c6154ce067bda88a8a19c1e6f52d2ecbdf4366823c7ccd287e1d8c99491e320 -->
# Module Specification

Module: nfs_mamont::consts::nlm
Rust File: src/consts/nlm.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **None**: The module contains only constant definitions and does not depend on any external crates or internal modules. It relies solely on built-in Rust primitive types (`u32`, `usize`).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module.
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the static contract and protocol identifiers for the Network Lock Manager (NLM) version 4. This module serves as the central source of truth for "magic numbers" required to construct, interpret, and dispatch NLM RPC messages within the larger NFS stack.

Inputs:
- None (Compile-time constants).

Outputs:
- `u32` values representing RPC program numbers, version numbers, and procedure identifiers (opcodes).
- `usize` values defining buffer size limits for strings and opaque handles.

Steps:
1. The module defines the standard RPC program number (`NLM_PROGRAM`) and version (`NLM_VERSION`).
2. It enumerates all procedure numbers (e.g., `NLMPROC4_LOCK`, `NLMPROC4_UNLOCK`) corresponding to the specific functions available in the NLM v4 protocol.
3. It defines size constraints (`LM_MAXSTRLEN`, `OPAQUE_HANDLE_SIZE`) used for memory allocation and data validation.

Edge Cases:
- None. The values are static and invariant at runtime.

Complexity:
- Time: O(1) (Direct memory access to constants).
- Space: O(1) (Stored in static memory/compiled binary).

Determinism:
- Deterministic

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- N/A (No dependencies).

---

## 4. Data Model

Entities:
- None (No structs or enums are defined in this module).

Relations:
- None.

Global Invariants:
- `NLM_PROGRAM` must be `100021` (Standard IANA assigned number for NLM).
- `NLM_VERSION` must be `4` (The specific protocol version supported).
- Procedure constants (`NLMPROC4_*`) must map uniquely to the operations defined in the NLM v4 specification.
- `LM_MAXSTRLEN` and `OPAQUE_HANDLE_SIZE` define the maximum capacity for specific data fields in the protocol.

---

## 5. Error Model

Error Types:
- None.

Error Propagation Strategy:
- N/A.

Recoverability:
- N/A.

Panics:
- Allowed: No
- Conditions: This module performs no computation and contains no fallible operations.

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

This module is used in order to provide the necessary protocol identifiers for implementing the Network Lock Manager (NLM) version 4, which is a side-band protocol used alongside NFS to manage file locking across a network. This system contains an NFS implementation that requires robust file locking capabilities to prevent data corruption when multiple clients access the same files. A typical usage scenario of the system involves an RPC server receiving a request; the server uses the constants defined here (specifically `NLM_PROGRAM` and `NLM_VERSION`) to verify that the request is intended for the Lock Manager service. Subsequently, the procedure number (e.g., `NLMPROC4_LOCK`) extracted from the request is matched against the constants in this module to dispatch the request to the correct internal handler (lock, unlock, test, etc.). Inside the system the following things happen and they use these constants to validate incoming packet headers, allocate buffers of size `LM_MAXSTRLEN` or `OPAQUE_HANDLE_SIZE` for client identifiers, and construct response packets with the correct procedure IDs. Without this module, the logic for encoding/decoding NLM messages would be littered with magic numbers, making the codebase unmaintainable and prone to implementation errors regarding the standard protocol.