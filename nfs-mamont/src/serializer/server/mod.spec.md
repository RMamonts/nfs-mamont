<!-- SPEC_HASH: 1a6182b77207606c8a15781b5b0c75bdca5940018270575f9c89bde0079f9e2b -->
# Module Specification

Module: nfs_mamont::serializer::server
Rust File: src/serializer/server/mod.rs

---

## 1. Dependencies

From the source code analysis, the following submodules are declared as dependencies/components of this module:

- **`mod mount`**: A private submodule containing serialization logic for the MOUNT protocol (status codes and procedure bodies).
- **`mod nfs`**: A private submodule containing serialization logic for the NFSv3 protocol (status codes and procedure bodies).
- **`mod nlm`**: A private submodule containing serialization logic for the NLM (Network Lock Manager) protocol (procedure responses).
- **`pub mod rpc`**: A public submodule containing serialization logic for generic RPC structures, specifically `OpaqueAuth`.
- **`pub mod serialize_struct`**: A public submodule containing the high-level `Serializer` struct responsible for orchestrating the serialization of full RPC replies.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To act as a namespace aggregator and access control layer for the server-side serialization components of the NFS stack. It groups the serializers for different protocols (NFS, MOUNT, NLM) and the RPC transport layer under a single parent module, while exposing only the high-level orchestration struct and generic RPC utilities to the outside world.

Inputs:
- None (This module only defines the module structure).

Outputs:
- A structured namespace `nfs_mamont::serializer::server` exposing `rpc` and `serialize_struct` publicly, while keeping `mount`, `nfs`, and `nlm` private.

Steps:
1. **Module Declaration**: The file declares the existence of submodules corresponding to the different protocols handled by the server.
2. **Visibility Control**:
 - It declares `rpc` and `serialize_struct` as `pub`, making them accessible to other parts of the crate (e.g., the RPC server listener).
 - It declares `mount`, `nfs`, and `nlm` without `pub`, restricting their access to within the `serializer::server` hierarchy. This suggests that the specific protocol serializers are implementation details used internally, likely by the `Serializer` in `serialize_struct`.

Edge Cases:
- None.

Complexity:
- Time: N/A (Compile-time structure).
- Space: N/A.

Determinism:
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::serializer::server::mount`**:
 - **`mount_stat`**: Provides the serialization for MOUNT protocol status codes.
 - **Namespace Organization**: Acts as a container for `dump`, `export`, and `mnt` serializers.

- **From `nfs_mamont::serializer::server::nlm`**:
 - **Response Serializers (`lock_res`, `unlock_res`, etc.)**: Provides the logic to convert NLM result structs into XDR bytes, handling specific union logic for the `TEST` procedure.

- **From `nfs_mamont::serializer::server::rpc`**:
 - **`auth`**: Provides the serialization for `OpaqueAuth`, which is a fundamental part of the RPC message header.

- **From `nfs_mamont::serializer::server::serialize_struct`**:
 - **`Serializer`**: The main entry point for serializing server replies. It likely utilizes the private `mount`, `nfs`, and `nlm` modules to serialize the body of the reply based on the procedure being invoked.

- **From `nfs_mamont::serializer::server::nfs`**:
 - **`error`**: Provides the serialization for NFSv3 status codes, which are the first field in every NFS response.

---

## 4. Data Model

Entities:
- None defined in this module.

Relations:
- **Aggregation**: This module aggregates the serialization logic for the MOUNT, NFS, NLM, and RPC protocols.

Global Invariants:
- The private visibility of `mount`, `nfs`, and `nlm` implies that external consumers should not need to manually invoke these specific serializers; they should rely on the public `Serializer` or `rpc` interfaces.

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

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **unify and structure the server-side serialization logic** for the NFS project. The project implements multiple distinct protocols (NFSv3, MOUNT, NLM) that run on top of ONC RPC. Each protocol has its own set of data structures and status codes that need to be converted into XDR byte streams. Without this module, these serializers would be scattered or lack a clear hierarchical relationship.

The system contains the complete machinery to convert high-level Rust domain objects (like `Nlm4LockRes` or `vfs::Error`) into the raw bytes required for network transmission. This module serves as the root of this machinery. It exposes the high-level `Serializer` (via `serialize_struct`), which acts as the facade for the system. It also exposes the `rpc` module for lower-level RPC specific structures (like authentication). Crucially, it encapsulates the protocol-specific details (`mount`, `nfs`, `nlm`), hiding them from the rest of the application. This design enforces a separation of concerns: the main server loop interacts with the generic `Serializer`, while the `Serializer` internally delegates to the specific protocol modules based on the RPC program number.

A typical usage scenario of the system involves the RPC server receiving a request and processing it, resulting in a high-level response object (e.g., an `NFS3ReadRes`). The server code creates a `Serializer` instance wrapping a network buffer. It calls `form_reply` on the `Serializer`. The `Serializer` (residing in the public `serialize_struct` submodule) inspects the result, determines it is an NFS response, and internally calls the private `nfs` module to write the status code and the file data. If the response were an NLM lock result, it would call the private `nlm` module.

Inside the system, the following things happen and they use this module:
- **Protocol Dispatching**: The `Serializer` uses the private modules (`nfs`, `mount`, `nlm`) as strategy implementations for different protocols. This module's structure dictates that these strategies are not accessible outside the `serializer::server` context, ensuring that all serialization goes through the validated `Serializer` logic.
- **Namespace Management**: By grouping `rpc` here, the system ensures that RPC-specific utilities (like `auth`) are co-located with the server logic that uses them, providing a clean `nfs_mamont::serializer::server::rpc` path for imports.

**Uncertainty**: The source code for `serialize_struct` was not fully provided, but its specification indicates it holds a `Serializer` struct with a `form_reply` method. It is assumed that `form_reply` is the primary user of the private modules (`mount`, `nfs`, `nlm`), as there is no other public interface exposed by this `mod.rs` file that would allow access to them.