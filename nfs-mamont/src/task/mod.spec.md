<!-- SPEC_HASH: 53a8a1ef79fc25b889f65cdfea07247186ffdcccbdbef61c212173e14dbf1555 -->
# Module Specification

Module: nfs_mamont::task
Rust File: src/task/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::allocator::Buffer`**: Used as a generic bound (`B: Buffer`) for the `ProcResult` enum. This allows the NFS result variant (`Nfs3`) to hold data buffers that are compatible with the server's custom memory allocator, enabling zero-copy or efficient data transfer strategies.
- **`crate::mount::MountRes`**: Used as the payload for the `Mount` variant of `ProcResult`. It represents the specific result structure returned by the MOUNT protocol service (e.g., authentication status, file handle).
- **`crate::nlm::NlmRes`**: Used as the payload for the `Nlm4` variant of `ProcResult`. It represents the specific result structure returned by the Network Lock Manager service (e.g., lock granted, lock denied).
- **`crate::rpc::Error`**: Used within the `ProcReply` struct to represent RPC-level failures. This allows the task system to distinguish between a successful protocol result and a failure that occurred during RPC processing (e.g., authentication failure, program mismatch).
- **`crate::vfs::NfsRes`**: Used as the payload for the `Nfs3` variant of `ProcResult`. It represents the specific result structure returned by the Virtual File System for NFSv3 operations (e.g., file attributes, read data).
- **`crate::task::connection`**: Declared as a public submodule. It is re-exported here to provide a unified namespace for task-related logic, specifically handling per-client connection lifecycles.
- **`crate::task::global`**: Declared as a public submodule. It is re-exported here to provide access to global task implementations (VFS pool, MOUNT, NLM) that are shared across connections.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define a unified type system for RPC replies that abstracts over the specific protocol (NFS, MOUNT, NLM) being handled.
- To associate a protocol-specific result with the RPC transaction identifier (`xid`) required for the client to match the reply to its request.
- To provide a namespace (`task`) that aggregates connection-specific and global task logic.

Inputs:
- N/A (This module defines data types and namespaces; it does not consume runtime inputs directly).

Outputs:
- **`ProcResult<B>` Enum**: A tagged union capable of holding the successful result of any supported high-level RPC program.
- **`ProcReply<B>` Struct**: A container combining an RPC transaction ID with a `Result` that either holds a `ProcResult` (success) or an `rpc::Error` (failure).

Steps:
1. **Enum Definition**: The `ProcResult` enum is defined with three variants, each wrapping a `Box` containing the specific result type from a different protocol module (`vfs`, `mount`, `nlm`).
2. **Struct Definition**: The `ProcReply` struct is defined with two fields: `xid` (a 32-bit unsigned integer) and `proc_result` (a `Result` type).

Edge Cases:
- N/A (Static type definitions).

Complexity:
- **Time**: O(1) (Type definitions have no runtime cost).
- **Space**: The size of `ProcReply` is determined by the size of `u32` plus the size of `Result<ProcResult, Error>`. The use of `Box` in `ProcResult` variants ensures the enum size remains manageable regardless of the size of the underlying result structs.

Determinism:
- Deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
 - **Result Aggregation**: The module relies on `vfs::NfsRes` to provide the specific success/failure types for all NFSv3 procedures. By wrapping `Box<NfsRes<B>>` in `ProcResult::Nfs3`, this module allows the RPC layer to treat the complex outcome of any filesystem operation as a single enum variant.
- **From `nfs_mamont::mount`**:
 - **Protocol Encapsulation**: The module uses `mount::MountRes` to represent the outcome of MOUNT protocol procedures. This allows the server to return mount-specific data (like file handles) through the generic `ProcReply` pipeline.
- **From `nfs_mamont::nlm`**:
 - **Lock Status Reporting**: The module uses `nlm::NlmRes` to represent the outcome of NLM procedures. This integrates the locking manager's status (granted/denied) into the unified reply mechanism.
- **From `nfs_mamont::rpc`**:
 - **Error Handling**: The module uses `rpc::Error` to populate the `Err` variant of `proc_result` in `ProcReply`. This bridges the gap between protocol-specific logic and the generic ONC RPC error reporting standard.

---

## 4. Data Model

Entities:
- **`ProcResult<B: Buffer>`**: An enumeration representing the successful result of an RPC call.
 - `Nfs3(Box<NfsRes<B>>)`: Holds the result of an NFSv3 procedure.
 - `Mount(Box<MountRes>)`: Holds the result of a MOUNT procedure.
 - `Nlm4(Box<NlmRes>)`: Holds the result of an NLMv4 procedure.
- **`ProcReply<B: Buffer>`**: A structure representing a complete RPC reply message ready for serialization.
 - `xid: u32`: The transaction identifier from the original RPC call.
 - `proc_result: Result<ProcResult<B>, Error>`: The payload, which is either a successful protocol result or an RPC error.

Relations:
- **Composition**: `ProcReply` contains `Result<ProcResult<B>, Error>`.
- **Composition**: `ProcResult` contains `Box<NfsRes<B>>`, `Box<MountRes>`, or `Box<NlmRes>`.

Global Invariants:
- The `xid` in a `ProcReply` must match the `xid` of the corresponding RPC request received from the network (this invariant is maintained by the caller, not this module).

## 5. Error Model

Error Types:
- **`crate::rpc::Error`**: Represents errors that occur during RPC processing (e.g., authentication failure, version mismatch, garbage arguments).

Error Propagation Strategy:
- **Structural Wrapping**: Errors are propagated by placing them in the `Err` variant of the `proc_result` field within `ProcReply`. This allows the serialization layer to handle both success and failure cases uniformly.

Recoverability:
- **Context Dependent**: Recoverability is determined by the consumer of the `ProcReply` (typically the write task). An RPC error usually implies the request failed and should be reported to the client, but the connection remains open.

Panics:
- **Allowed**: No (This module only defines data structures).

---

## 6. Traits

List which external traits this module implements:
- None.

---

## 7. Overview

This module is used in order to **provide a unified interface for transmitting RPC responses** across different protocols within the NFS server. It solves the problem of type heterogeneity: the server handles multiple distinct protocols (NFS, MOUNT, NLM), each with its own complex result types, but the network transport layer needs a single, consistent type to serialize and send.

The system contains a multi-protocol RPC architecture where the `ReadTask` (in `connection`) dispatches requests to various services (VFS, MOUNT, NLM). These services return different result types (`NfsRes`, `MountRes`, `NlmRes`). This module defines `ProcResult` to act as a common "sum type" that can hold any of these. By wrapping these results in `Box`, it ensures that moving these large variants between tasks is efficient. The `ProcReply` struct then adds the necessary `xid` (transaction ID) context, creating a complete package that the `WriteTask` can serialize and send back to the client without needing to know which specific protocol was handled.

A typical usage scenario of the system involves a worker task finishing a request. For example, if a client requested a file read, the VFS returns an `NfsRes`. The task constructs `ProcResult::Nfs3(Box::new(nfs_res))`. It then wraps this in `ProcReply { xid: original_xid, proc_result: Ok(proc_result) }`. If an authentication error had occurred earlier, it would instead construct `ProcReply { xid: original_xid, proc_result: Err(rpc::Error::Auth(...)) }`. This `ProcReply` is then sent to the write task.

Inside the system, the following things happen and they use this module:
1. **Type Erasure/Abstraction**: The RPC layer uses `ProcReply` to handle responses generically. The serializer only needs to know how to handle `ProcReply`, not the specific details of `NfsRes` or `MountRes`.
2. **Protocol Multiplexing**: The `ProcResult` enum enables the server to support multiple RPC programs (NFS, MOUNT, NLM) simultaneously over the same port and connection infrastructure, as the reply pipeline is agnostic to the specific variant contained within.

The critical aspect of this module is the **standardization of the reply envelope**, ensuring that the diverse outcomes of the server's subsystems are normalized into a format suitable for network transmission.