<!-- SPEC_HASH: 1a6182b77207606c8a15781b5b0c75bdca5940018270575f9c89bde0079f9e2b -->
# Module Specification

Module: nfs_mamont::serializer::server
Rust File: src/serializer/server/mod.rs

---

## 1. Dependencies

From the code analysis, this module acts as a parent module and does not import external crates or other modules via `use` statements. Instead, it declares the following submodules to organize the serialization logic:

* **`mod mount;`**:
 * Purpose: Encapsulates the serialization logic for the MOUNT protocol (e.g., export lists, mount status).
* **`mod nfs;`**:
 * Purpose: Encapsulates the serialization logic for the NFSv3 protocol (e.g., file attributes, directory entries).
* **`mod nlm;`**:
 * Purpose: Encapsulates the serialization logic for the Network Lock Manager (NLM) protocol (e.g., lock results, test responses).
* **`mod rpc;`**:
 * Purpose: Encapsulates the serialization logic for generic ONC RPC protocol structures (e.g., authentication credentials).
* **`mod serialize_struct;`**:
 * Purpose: Contains the high-level orchestration logic (`Serializer`) that constructs the complete RPC reply messages, manages buffering, and handles the Record Marking Standard.
* **`#[cfg(test)] mod tests;`**:
 * Purpose: Contains unit tests specific to the server-side serialization components.

---

## 2. Mechanics

This module implements a **Namespace Aggregation** mechanism.

Intent:
- To provide a hierarchical structure for the server-side serialization subsystem, separating concerns by protocol (NFS, MOUNT, NLM) and by architectural layer (RPC framing vs. specific procedure payloads).

Inputs:
- None. This module does not define functions or accept inputs directly.

Outputs:
- A module namespace exposing the submodules `mount`, `nfs`, `nlm`, `rpc`, and `serialize_struct` to the rest of the crate.

Steps:
1. The Rust compiler processes the `mod` declarations.
2. It locates the corresponding files (e.g., `mount.rs`, `nfs/mod.rs`) or inline modules.
3. It makes the public items within those submodules accessible via paths like `nfs_mamont::serializer::server::rpc::auth`.

Edge Cases:
- None. This is a structural module.

Complexity:
- Time: N/A (Compile-time organization).
- Space: N/A.

Determinism:
- Deterministic. The module structure is fixed at compile time.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

Since this module aggregates its dependencies (submodules), the following mechanisms provided by them constitute the functionality of this subsystem:

- **From `nfs_mamont::serializer::server::serialize_struct`**:
 - **`Serializer::form_reply`**: The primary entry point for converting a high-level `ProcReply` into a network stream. It handles RPC headers, status mapping, and dispatching to specific protocol serializers.
 - **`WriteBuffer`**: Manages the internal buffering of XDR metadata and handles the Record Marking Standard (RMS) framing.
 - **`send_inner_with_buffer`**: Enables zero-copy transmission of file data for NFS READ operations using vectored I/O.

- **From `nfs_mamont::serializer::server::rpc`**:
 - **`auth`**: Serializes the `OpaqueAuth` structure, which is required for the verifier field in RPC reply headers.

- **From `nfs_mamont::serializer::server::nlm`**:
 - **`lock_res`, `unlock_res`, `cancel_res`, `test_res`**: Serialize the results of NLM procedures. `test_res` specifically handles conditional serialization of lock holder data based on the status code.

- **From `nfs_mamont::serializer::server::mount`**:
 - **`mount_stat`**: Serializes the status codes for MOUNT protocol procedures.

- **From `nfs_mamont::serializer::server::nfs`**:
 - **`error`**: Serializes NFS-specific error codes into the wire format.

---

## 4. Data Model

Entities:
- None defined in this module.

Relations:
- **Parent-Child**: This module is the parent of `mount`, `nfs`, `nlm`, `rpc`, and `serialize_struct`.

Global Invariants:
- None defined in this module.

---

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

---

## 7. Overview

This module is used in order to **organize the server-side serialization subsystem of the NFS server**, providing a clean namespace that separates the generic RPC framing logic from the specific data serialization requirements of the NFS, MOUNT, and NLM protocols.

This system contains a **layered architecture** where the `serialize_struct` submodule acts as the orchestrator. It relies on the `rpc` submodule to handle protocol-level framing (like authentication) and delegates the encoding of specific procedure results to the `nfs`, `mount`, and `nlm` submodules. This separation allows the codebase to manage the complexity of supporting multiple distinct protocols (NFSv3, MOUNT v3, NLM v4) within a unified RPC framework.

A typical usage scenario of the system involves the server's task processor generating a `ProcReply` containing the result of a file operation. The main server logic invokes `Serializer::form_reply` from the `serialize_struct` submodule. This mechanism determines the protocol type (e.g., NFSv3) and calls the corresponding serializer function (e.g., from the `nfs` submodule) to write the specific payload bytes into a buffer, which is then transmitted to the client.

Inside the system the following things happen and they use **the `std::io::Write` trait as a common abstraction** for all serialization targets. The `serialize_struct` module manages a `WriteBuffer` that implements `Write`, allowing the low-level serializers in `nfs`, `nlm`, etc., to simply "write" data without needing to know about network sockets, buffering strategies, or the Record Marking Standard. This module effectively serves as the root of this specific tree of serialization logic.