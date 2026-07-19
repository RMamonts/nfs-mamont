<!-- SPEC_HASH: 7d383c7055711953e2ec478be0b894249083345f14b5ee7883f4fc4b1b62169d -->
# Module Specification

Module: nfs_mamont::vfs::commit
Rust File: src/vfs/commit.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import `WccData` and `Error`. `WccData` is included in both `Success` and `Fail` results to provide the client with weak cache consistency information (pre- and post-operation attributes). `Error` is used in `Fail` to specify the reason for the operation failure.
- **`super::file`**: Used to import `Handle`. This type is used in `Args` to identify the specific file on which the commit operation is to be performed.
- **`vfs::write`**: Used to import `Verifier`. The `Success` struct contains a `Verifier` which corresponds to the write operation that previously committed the data to unstable storage. This allows the client to verify server state continuity.
- **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` attribute macro on the `Commit` trait. This transforms the async trait into an object-safe trait that implements `Send`, allowing it to be used as a trait object in multi-threaded, asynchronous contexts (e.g., dynamic dispatch across threads).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `COMMIT` procedure.
- To provide a contract for forcing previously written data (which may be in volatile cache) to stable storage (disk).
- To facilitate cache consistency and crash recovery verification for clients using unstable writes.

Inputs:
- **`Args`**: A structure containing:
 - `file`: The `file::Handle` identifying the target file.
 - `offset`: The `u64` byte offset in the file where the flush range begins.
 - `count`: The `u32` number of bytes to flush. If `0`, the range extends from `offset` to the end of the file.

Outputs:
- **`Result<Success, Fail>`**:
 - **`Success`**: Contains `file_wcc` (updated file attributes for cache consistency) and `verifier` (a cookie confirming the data state).
 - **`Fail`**: Contains `error` (the specific VFS error encountered) and `file_wcc` (file attributes, typically pre-operation state, to allow cache updates).

Steps:
1. **Trait Definition**: The module defines the `Commit` trait using `#[trait_variant::make(Send)]`. This makes the trait usable as a `dyn Commit` object that can be sent between threads.
2. **Argument Specification**: The `Args` struct defines the range of data to be committed. The logic for handling `count == 0` (flush to EOF) is defined here as a contract for the implementer.
3. **Result Definition**: The `Success` struct returns a `verifier`. This mechanism links the commit operation to a previous write operation, allowing the client to detect if the server has rebooted and lost its cache (since the verifier would change or become invalid).
4. **Error Handling**: The `Fail` struct ensures that even if the commit fails, the client receives `WccData` to synchronize its cache, preventing stale attribute data.

Edge Cases:
- **Flush to EOF**: If `Args::count` is `0`, the implementation must flush all data from `Args::offset` to the end of the file.
- **Verifier Consistency**: The `verifier` returned in `Success` must match the verifier associated with the data being committed. If the server rebooted, the verifier might differ, signaling to the client that the data was lost.

Complexity:
- Time: O(1) for interface definition. The actual complexity of the `commit` operation depends on the implementation (typically O(N) where N is the amount of data to flush, or O(1) if metadata only).
- Space: O(1) for the interface structures.

Determinism:
- Deterministic (Interface definition).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: Provides the opaque identifier for the file. The `Commit` operation relies on this handle to locate the file context within the VFS without exposing internal paths or inodes.
- **From `nfs_mamont::vfs`**:
 - **`WccData`**: Provides the mechanism for Weak Cache Consistency. By including `WccData` in both `Success` and `Fail`, the module ensures the client can update its attribute cache regardless of the operation's outcome, which is critical for NFS performance.
 - **`Error`**: Provides the standardized error enumeration used to report failures (e.g., `IO`, `StaleFile`, `Access`) back to the client.
- **From `nfs_mamont::vfs::write`**:
 - **`Verifier`**: Provides the "cookie" mechanism. The `Commit` interface returns this to confirm that the unstable data previously written (and associated with a specific verifier) has now been made stable. This is essential for the client-side crash detection logic.

---

## 4. Data Model

Entities:
- **`Args`**: The input parameters for a commit request, defining the file handle and the byte range `[offset, offset + count)` (or `[offset, EOF)` if count is 0).
- **`Success`**: The result of a successful commit, containing the `WccData` (post-operation attributes) and the `Verifier`.
- **`Fail`**: The result of a failed commit, containing the `vfs::Error` and the `WccData` (attributes, usually pre-operation).

Relations:
- **Composition**: `Args` contains a `file::Handle`.
- **Association**: `Success` and `Fail` both contain `vfs::WccData`.
- **Dependency**: `Success` contains `vfs::write::Verifier`.

Global Invariants:
- **Range Semantics**: If `Args::count` is `0`, the range to commit is implicitly defined as `Args::offset` to the end of the file.
- **Verifier Return**: The `verifier` returned in `Success` must be the one associated with the committed data, allowing the client to verify consistency with a previous `WRITE` response.

## 5. Error Model

Error Types:
- `vfs::Error` (wrapped in `Fail`)

Error Propagation Strategy:
- The `commit` method returns a `Result<Success, Fail>`. Errors are wrapped in the `Fail` struct, which includes the specific `vfs::Error` and `file_wcc` to allow the client to update its cache even on failure.

Recoverability:
- Recoverable. The client receives the error and can retry the commit or take corrective action based on the error code (e.g., `StaleFile` implies the file handle is invalid).

Panics:
- Allowed: No (in the interface definition).
- Conditions: The interface itself does not panic. Implementations may panic, but the trait definition does not specify panic conditions.

---

## 6. Traits

List which external traits this module implements:
- **`Send`**: Implemented for `Commit` via the `#[trait_variant::make(Send)]` attribute macro.

List which traits this module defines:
- **`Commit`**: An asynchronous trait defining the contract for flushing data to stable storage. It requires the implementer to handle `Args` and return a `Result<Success, Fail>`.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here you MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to define the interface for ensuring data durability in the NFSv3 server implementation. The system contains a Virtual File System (VFS) layer that supports "unstable" writes—writes where the server acknowledges receipt before data is physically written to disk—to improve performance. However, this creates a risk of data loss if the server crashes before the data is flushed. The `Commit` module provides the necessary mechanism for the client to explicitly request that this cached data be synchronized to stable storage.

A typical usage scenario of the system involves a client performing a `WRITE` operation with `StableHow::Unstable`. The server accepts the data into a cache and returns a `Verifier` (defined in `vfs::write`). Later, the client issues a `COMMIT` request using the `Commit` trait defined in this module, specifying the file handle and the byte range. The VFS backend implementation of `Commit` flushes the specified range to disk and returns a `Success` result containing the `Verifier`. The client compares this verifier with the one from the write; if they match, the client knows the server has not rebooted and the data is safe. If they differ, the client knows the data was lost and must re-write it.

Inside the system, the following things happen and they use this module: The RPC layer receives a `COMMIT` request from the network and decodes it into `Args`. It then invokes the `commit` method on the VFS backend. The backend performs the I/O operation (e.g., `fdatasync` or writing specific ranges). The `WccData` included in the response allows the client to update its attribute cache without issuing a separate `GETATTR` request. Without this module, the VFS layer would lack a standardized way to handle the transition of data from unstable to stable states, preventing the server from offering high-performance unstable writes while still guaranteeing eventual data consistency.