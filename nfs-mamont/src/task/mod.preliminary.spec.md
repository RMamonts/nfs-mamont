<!-- SPEC_HASH: 53a8a1ef79fc25b889f65cdfea07247186ffdcccbdbef61c212173e14dbf1555 -->
# Module Specification

Module: nfs_mamont::task
Rust File: src/task/mod.rs

---

## 1. Dependencies

From the provided context and dependency facts:

- **`crate::allocator::Buffer`**: Used as a generic constraint (`B: Buffer`) for `ProcResult`. This ensures that data-heavy results (specifically NFSv3 results) can utilize the custom memory management system defined by the allocator, enabling zero-copy or optimized memory handling strategies.
- **`crate::mount::MountRes`**: Used as a payload variant within `ProcResult`. It represents the specific result types defined by the MOUNT protocol (e.g., export lists, mount status).
- **`crate::nlm::NlmRes`**: Used as a payload variant within `ProcResult`. It represents the result types defined by the Network Lock Manager (NLM) protocol (e.g., lock status, blocked requests).
- **`crate::rpc::Error`**: Used within the `ProcReply` struct. It provides the error variant for the `Result` type, allowing the task infrastructure to propagate RPC-level failures (e.g., authentication errors, program mismatches) alongside successful protocol results.
- **`crate::vfs::NfsRes<B>`**: Used as a payload variant within `ProcResult`. It represents the result of NFSv3 file system operations (e.g., read, write, lookup). It is generic over `Buffer` to hold file data directly in the allocated memory.

*Assumption*: Specifications for `vfs`, `mount`, and `nlm` were not fully provided, but based on the `*.facts.json` and naming conventions, `NfsRes`, `MountRes`, and `NlmRes` are enumerations representing the union of all possible procedure results for their respective protocols.

---

## 2. Mechanics

**Intent:**
The module serves as a type-level bridge between the specific protocol implementations (NFS, Mount, NLM) and the generic RPC transport layer. Its purpose is to define a unified container (`ProcReply`) that can hold the result of any supported RPC program, associating it with the transaction ID (`xid`) required for routing the response back to the client.

**Inputs:**
- N/A (This module defines data structures; it does not consume inputs directly).

**Outputs:**
- **`ProcResult<B>`**: A tagged union enum wrapping the specific result types of the supported protocols.
- **`ProcReply<B>`**: A struct combining an RPC transaction ID with a `Result` containing either a `ProcResult` or an RPC `Error`.

**Steps:**
1. Define `ProcResult<B>` as an enum with variants for `Nfs3`, `Mount`, and `Nlm4`.
2. Wrap the specific result types (`NfsRes<B>`, `MountRes`, `NlmRes`) in `Box` within the enum variants to ensure the enum size remains manageable (pointer-sized) regardless of the size of the contained data.
3. Define `ProcReply<B>` containing a `xid` field (`u32`) and a `proc_result` field (`Result<ProcResult<B>, Error>`).

**Edge Cases:**
- N/A (Static definitions).

**Complexity:**
- Time: N/A
- Space: `ProcResult` is the size of a pointer plus a discriminant due to the usage of `Box`. `ProcReply` adds the size of a `u32` and the `Result` wrapper.

**Determinism:**
- Deterministic (Type definitions are static).

---

## 3. Dependency Mechanics

- **`crate::rpc::Error`**: The `ProcReply` struct relies on the `Error` enum from the RPC module to represent failure states. This creates a dependency where the task layer delegates the definition of "what went wrong" to the lower-level RPC protocol layer, ensuring that error codes match the ONC RPC standard.
- **`crate::vfs::NfsRes<B>`**: The `ProcResult::Nfs3` variant propagates the generic `Buffer` constraint from the VFS layer. This mechanism allows the task layer to handle NFS results that are backed by custom memory (e.g., memory-mapped or pooled buffers) without knowing the details of the memory implementation.
- **`crate::allocator::Buffer`**: The `Buffer` trait acts as a boundary for the memory used in NFS results. The task module enforces that any `NfsRes` contained within a `ProcReply` must adhere to this interface, ensuring compatibility with the rest of the server's memory architecture.

---

## 4. Data Model

**Entities:**
- **`ProcResult<B>`**: A sum type representing the successful outcome of an RPC call. It is generic over `B` (Buffer).
  - `Nfs3(Box<NfsRes<B>>)`: Holds the result of an NFSv3 procedure.
  - `Mount(Box<MountRes>)`: Holds the result of a MOUNT protocol procedure.
  - `Nlm4(Box<NlmRes>)`: Holds the result of an NLMv4 procedure.
- **`ProcReply<B>`**: A structure representing the complete response message ready for serialization.
  - `xid: u32`: The transaction identifier from the client's request.
  - `proc_result: Result<ProcResult<B>, Error>`: The payload, which is either a successful protocol result or an RPC error.

**Relations:**
- `ProcReply` → `ProcResult` (Composition via `proc_result`).
- `ProcReply` → `Error` (Composition via `proc_result`).
- `ProcResult` → `NfsRes<B>` (Composition via `Nfs3` variant).
- `ProcResult` → `MountRes` (Composition via `Mount` variant).
- `ProcResult` → `NlmRes` (Composition via `Nlm4` variant).

**Global Invariants:**
- The `xid` in `ProcReply` must correspond to the `xid` of the request being processed (enforced by the caller, not the type).
- `ProcResult` variants are mutually exclusive; a single reply can only represent the result of one protocol program.

---

## 5. Error Model

**Error Types:**
- **`crate::rpc::Error`**: The module does not define its own errors but re-exports or uses the error type from the `rpc` module. This covers protocol-level errors like version mismatches, authentication failures, and procedure unavailability.

**Error Propagation Strategy:**
- **Composition**: Errors are embedded directly into the `ProcReply` struct via the `Result` type. This allows the task handler to return a single type (`ProcReply`) regardless of whether the operation succeeded or failed.

**Recoverability:**
- N/A (Data definition).

**Panics:**
- Allowed: No (No code in this module triggers panics).

---

## 6. Traits

The module defines and implements no external traits. It acts as a consumer of the `Buffer` trait (via the generic parameter `B`).

---

## 7. Overview

This module is used in order to **unify the dispatch and response handling of a multi-protocol NFS server**. An NFS server typically handles three distinct protocols: the main NFS file operations (NFSv3), the mounting protocol (MOUNT), and the network locking protocol (NLM). Each of these protocols has its own distinct set of procedures and result types.

This system contains a **polymorphic response container** (`ProcReply`) that abstracts over the specific protocol being handled. It allows the RPC transport layer to treat the outcome of any procedure—whether it's reading a file via NFS, mounting a directory via MOUNT, or acquiring a lock via NLM—as a single entity that can be serialized and sent back to the client.

A typical usage scenario of the system involves a connection handler task receiving an RPC request. The handler inspects the "program number" in the request header to determine which protocol service to invoke.
1. If it is an NFS request, the handler calls the VFS, which returns an `NfsRes<B>`. The handler wraps this in `ProcResult::Nfs3`.
2. If it is a MOUNT request, the handler calls the Mount service, which returns a `MountRes`. The handler wraps this in `ProcResult::Mount`.
3. The handler then constructs a `ProcReply` containing the request's `xid` and the wrapped result.
4. This `ProcReply` is passed to the serialization logic to be converted into the byte stream sent over the network.

Inside the system, the following things happen and they use this module:
- **Task Spawning**: The `connection` submodule (assumed based on file structure) spawns asynchronous tasks for each client connection. These tasks produce `ProcReply` instances.
- **Result Aggregation**: By boxing the protocol results (`Box<NfsRes<B>>`, etc.), the module ensures that moving these results between asynchronous tasks or sending them through channels is efficient, as only the pointer is moved rather than large data structures.
- **Generic Memory Handling**: By propagating the `Buffer` generic, the module ensures that file data read from the disk (via VFS) can reside in the specific memory pool allocated by the `allocator` module, avoiding unnecessary copies.