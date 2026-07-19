<!-- SPEC_HASH: 59997be779fc1744ee4890b10b2140252e49d046c490471290449959cddfa2ae -->
# Module Specification

Module: nfs_mamont::mount::umnt
Rust File: src/mount/umnt.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::net::SocketAddr`**: Used to identify the network address (IP and port) of the client requesting the unmount operation. This is required because the mount table state is typically keyed by the client's address to distinguish between different clients mounting the same path.
- **`crate::vfs::file`**: Used to import the `file::Path` type. This type is utilized in the `Args` struct to represent the directory path string, ensuring that the path conforms to the server's validation rules (e.g., length limits) before being processed by the mount logic.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the MOUNT protocol version 3 `UMNT` procedure (Procedure 3), as specified in RFC 1813.
- To provide a structured, type-safe representation of the unmount arguments (`Args`) and an asynchronous trait (`Umnt`) that implementers must use to handle unmount requests.
- To enforce the contract that an unmount operation is idempotent and does not return protocol-level errors, as per the specification.

Inputs:
- `args: Args`: A structure containing the `dirpath` (`file::Path`) of the directory to be unmounted.
- `client_addr: SocketAddr`: The socket address of the client issuing the request.

Outputs:
- `()`: The `umnt` function returns nothing. Success is indicated by the completion of the asynchronous execution. The RFC specifies that no MOUNT protocol errors can be returned from this procedure.

Steps:
1. **Argument Construction**: The caller constructs an `Args` struct containing the `file::Path` of the export to be unmounted.
2. **Trait Invocation**: The `umnt` method on the `Umnt` trait implementation is invoked with the `Args` and the `client_addr`.
3. **State Mutation (Implicit)**: The implementation of the trait is expected to locate the mount list entry corresponding to the provided `client_addr` and `dirpath` and remove it.
4. **Completion**: The operation completes, returning `()` to the RPC layer, which then sends a void response to the client.

Edge Cases:
- **Unmounting Non-Existent Mount**: If the client attempts to unmount a path that is not currently mounted (or was already unmounted), the implementation must handle this gracefully. Since the interface returns `()` and the RFC states no errors are returned, this operation should be treated as a no-op rather than an error condition.
- **Invalid Path**: The `file::Path` type ensures that the `dirpath` is structurally valid (e.g., not empty, within length limits) before it reaches the `umnt` logic.

Complexity:
- Time: O(1) for the interface definition. The complexity of the actual unmount operation depends on the implementation of the `Umnt` trait (e.g., hash map lookup).
- Space: O(1) for the `Args` struct.

Determinism:
- Deterministic. The interface defines a clear input-output contract.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
 - **`Path`**: This module relies on the `Path` struct to encapsulate the directory path. The `Path` struct provides validation guarantees (checked at construction time) that the `dirpath` string is non-empty and within the maximum length limit (`MAX_PATH_LEN`). This ensures that the `umnt` operation does not need to re-validate basic string constraints, delegating that concern to the type system.

---

## 4. Data Model

Entities:
- **`Args`**: A structure representing the arguments for the unmount procedure.
 - `dirpath`: `file::Path` — The server pathname of the directory to be unmounted.
- **`Umnt`**: An asynchronous trait defining the unmount operation.
 - `umnt`: An async function taking `Args` and `SocketAddr`.

Relations:
- **Composition**: `Args` composes `file::Path`.

Global Invariants:
- **Path Validity**: The `dirpath` field within `Args` is guaranteed to be a valid `file::Path` instance, meaning it satisfies the length and non-emptiness constraints defined in the `vfs::file` module.

## 5. Error Model

Error Types:
- None defined in this module.

Error Propagation Strategy:
- **Void Return**: The `umnt` function signature returns `()`, not a `Result`. According to RFC 1813, there are no MOUNT protocol errors specific to the `UMNT` procedure. Any internal errors (e.g., database failures in the implementation) would likely result in a panic or a generic RPC failure, but are not exposed to the client via the MOUNT protocol status field.

Recoverability:
- **Idempotency**: The interface implies idempotency. Calling `umnt` multiple times for the same path and client should not result in an error state, allowing the client to retry or issue the call without fear of protocol-level rejection.

Panics:
- **Allowed**: No. The interface itself does not panic. However, the underlying implementation of the `Umnt` trait may panic if internal invariants are violated (e.g., memory corruption), though this is not part of the defined interface behavior.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`Umnt`**: The trait defining the contract for the Unmount operation. It is marked with `#[trait_variant::make(Send)]`, implying that the trait object generated from it is safe to send across threads.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **define the interface for the cleanup phase of the MOUNT protocol lifecycle** within the `nfs_mamont` NFS server. The system contains a complex architecture where the MOUNT protocol is responsible for establishing the initial relationship between a client and a server export (mapping a server path to a file handle). The `UMNT` procedure, defined here, serves as the inverse operation: it allows the client to explicitly notify the server that it is finished with a specific export.

A typical usage scenario of the system involves a client shutting down or an administrator unmounting a filesystem on the client side. The client sends an RPC request corresponding to the `UMNT` procedure. The RPC layer deserializes the request into the `Args` struct (containing the `file::Path`) and identifies the client's `SocketAddr`. It then invokes the `umnt` method on the service implementing the `Umnt` trait. The service uses this information to remove the entry from the internal mount table, freeing resources and ensuring that subsequent `MNT` requests from the same client are treated as fresh mounts.

Inside the system, the following things happen and they use this module:
1. **Resource Management**: The server maintains a list of active mounts to track which clients are accessing which exports. This module provides the necessary hook (`Umnt` trait) for the server to update this list when a client disconnects or unmounts.
2. **Protocol Compliance**: The NFSv3 specification (RFC 1813) mandates the existence of the `UMNT` procedure. This module ensures that the server adheres to the standard by providing the exact signature and semantics required, including the handling of `AUTH_UNIX` authentication requirements and the void return type.
3. **Type Safety**: By using `file::Path` in the arguments, this module ensures that the unmount logic operates on validated, sanitized path strings, preventing malformed input from affecting the server's internal state.

Without this module, the server would lack a standardized way to process unmount requests, potentially leading to memory leaks (stale mount entries) or protocol violations where clients are unable to cleanly terminate their session with the server.