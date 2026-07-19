<!-- SPEC_HASH: 679d76ca02e3723470563d85f21cacf418d8295a949decc8f2d264baac73d7ce -->
# Module Specification

Module: nfs_mamont::parser
Rust File: src/parser/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`std::future::Future`**: Used to define the asynchronous behavior of the `proc_nested_errors` helper function. It allows the function to accept any future that resolves to a type `T` and returns a `Result` handling the execution and potential failure.
- **`crate::allocator::Buffer`**: Used as a generic bound `B` for the `ArgWrapper` and `NfsArgWrapper` structs. This allows the parser to return arguments that may contain references to memory buffers (e.g., for `WRITE` operations) managed by the server's custom allocator, ensuring type safety and memory control.
- **`crate::mount::{mnt, umnt}`**: Used to import the specific argument types for the MOUNT protocol procedures (`mnt::Args`, `umnt::Args`). These are used within the `MountArguments` enum variants to provide strongly-typed payloads for MOUNT service handlers.
- **`crate::nlm::procedures::{cancel::Nlm4CancelArgs, lock::Nlm4LockArgs, test::Nlm4TestArgs, unlock::Nlm4UnlockArgs}`**: Used to import the specific argument types for the NLMv4 protocol procedures. These are used within the `NlmArguments` enum variants to provide strongly-typed payloads for NLM service handlers.
- **`crate::rpc::{Error, OpaqueAuth}`**: Used to define the `Result` type alias and the authentication fields within `RpcHeader`. `Error` is the canonical error type for RPC operations, and `OpaqueAuth` wraps the authentication credentials and verifier from the RPC message header.
- **`crate::vfs::{access, commit, create, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node, path_conf, read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink, write}`**: Used to import the specific argument types for all NFSv3 protocol procedures. These are used within the `NfsArguments` enum variants to provide strongly-typed payloads that map directly to the inputs required by the VFS trait implementations.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

### Mechanism 1: Nested Error Handling (`proc_nested_errors`)

**Intent:**
- To provide a utility for chaining asynchronous operations where the success of a subsequent operation determines whether a previous error should be discarded or preserved.

**Inputs:**
- `error: Error`: The original error to be returned if the future fails.
- `future: impl Future<Output = Result<T>>`: An asynchronous operation to execute.

**Outputs:**
- `Error`: The resulting error.

**Steps:**
1. The function awaits the `future`.
2. If the future resolves to `Ok(_)`, the result is discarded, and the original `error` is returned.
3. If the future resolves to `Err(err)`, the new `err` is returned.

**Edge Cases:**
- **Future Success**: If the future completes successfully, the original `error` is preserved and returned.
- **Future Failure**: If the future fails, the new error overrides the original.

**Complexity:**
- **Time**: Determined by the execution time of the provided `future`.
- **Space**: O(1).

**Determinism:**
- **Deterministic**: The output is strictly determined by the outcome of the `future`.

### Mechanism 2: RPC Request Header Representation (`RpcHeader`)

**Intent:**
- To encapsulate the metadata extracted from the RPC message header, specifically the transaction ID and authentication context.

**Inputs:**
- N/A (Struct definition).

**Outputs:**
- N/A (Struct definition).

**Steps:**
- N/A (Struct definition).

**Edge Cases:**
- **Unused Verifier**: The `verf` field is currently marked `#[allow(dead_code)]` with a TODO comment indicating it should be used when authentication is provided.

**Complexity:**
- **Time**: O(1).
- **Space**: O(1).

**Determinism:**
- **Deterministic**.

### Mechanism 3: Protocol-Specific Argument Wrappers

**Intent:**
- To define structures that combine the generic `RpcHeader` with the specific arguments for each supported protocol (NFS, MOUNT, NLM). This allows the RPC dispatcher to pass a single object to the service layer that contains all necessary context (who sent the request, what they want to do).

**Inputs:**
- N/A (Struct definitions).

**Outputs:**
- `NfsArgWrapper<B>`: Combines `RpcHeader` with `NfsArguments<B>`.
- `MountArgWrapper`: Combines `RpcHeader` with `MountArguments`.
- `NlmArgWrapper`: Combines `RpcHeader` with `NlmArguments`.

**Steps:**
- N/A (Struct definitions).

**Edge Cases:**
- None.

**Complexity:**
- **Time**: O(1).
- **Space**: O(1) plus the size of the boxed arguments.

**Determinism:**
- **Deterministic**.

### Mechanism 4: Generic Argument Wrapper (`ArgWrapper`)

**Intent:**
- To provide a unified container for RPC arguments that can hold any of the supported protocol argument types (`Nfs3`, `Mount`, `Nlm4`). This is used by generic message consumers that accept multiple protocols.

**Inputs:**
- N/A (Struct definition).

**Outputs:**
- `ArgWrapper<B>`: A struct containing `header` and `proc`.

**Steps:**
- N/A (Struct definition).

**Edge Cases:**
- None.

**Complexity:**
- **Time**: O(1).
- **Space**: O(1) plus the size of the boxed `ProcArguments`.

**Determinism:**
- **Deterministic**.

### Mechanism 5: Error Association (`ErrorWrapper`)

**Intent:**
- To associate an RPC `Error` with a specific Transaction ID (`xid`). This is necessary for constructing rejection messages where the server must inform the client which specific request failed.

**Inputs:**
- N/A (Struct definition).

**Outputs:**
- `ErrorWrapper`: A struct containing `xid` and `error`.

**Steps:**
- N/A (Struct definition).

**Edge Cases:**
- **Missing XID**: The `xid` is `Option<u32>`, allowing for errors that occur before a full header is parsed or where the XID is not relevant.

**Complexity:**
- **Time**: O(1).
- **Space**: O(1).

**Determinism:**
- **Deterministic**.

### Mechanism 6: Procedure Argument Enumerations

**Intent:**
- To enumerate the specific arguments for every procedure supported by the server, grouped by protocol. This maps the integer procedure numbers found in the RPC header to concrete Rust types.

**Inputs:**
- N/A (Enum definitions).

**Outputs:**
- `ProcArguments<B>`: Enum with variants `Nfs3`, `Mount`, `Nlm4`.
- `NfsArguments<B>`: Enum with variants for all NFSv3 procedures (e.g., `Read(read::Args<B>)`).
- `MountArguments`: Enum with variants for MOUNT procedures (e.g., `Mount(mnt::Args)`).
- `NlmArguments`: Enum with variants for NLM procedures (e.g., `Lock(Nlm4LockArgs)`).

**Steps:**
- N/A (Enum definitions).

**Edge Cases:**
- **Null Procedures**: All enums include a `Null` variant representing procedure 0.

**Complexity:**
- **Time**: O(1).
- **Space**: O(1) plus the size of the contained argument structs.

**Determinism:**
- **Deterministic**.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::allocator`**:
 - **`Buffer` Trait**: The `ArgWrapper` and `NfsArgWrapper` structs are generic over `B: Buffer`. This allows the parser to return arguments that hold references to memory buffers (e.g., for `WRITE` operations) managed by the server's custom allocator, ensuring type safety and memory control.

- **From `nfs_mamont::vfs`**:
 - **Procedure Arguments**: The `NfsArguments` enum variants (e.g., `Read(read::Args<B>)`) directly embed the argument types defined in the VFS sub-modules (e.g., `vfs::read::Args`). This creates a direct dependency path from the parser output to the VFS trait inputs.
 - **`Error` Enum**: The `Result` type alias uses `rpc::Error`, ensuring that parsing errors are propagated using the standard RPC error type.

- **From `nfs_mamont::rpc`**:
 - **`OpaqueAuth`**: The `RpcHeader` uses `OpaqueAuth` to represent authentication data.
 - **`Error` Enum**: The `Result` type alias uses `rpc::Error`, ensuring that parsing errors are propagated using the standard RPC error type.

- **From `nfs_mamont::mount`**:
 - **Procedure Arguments**: The `MountArguments` enum variants (e.g., `Mount(mnt::Args)`) embed the argument types defined in the `mount` crate (e.g., `mnt::Args`), linking the parser to the MOUNT service.

- **From `nfs_mamont::nlm`**:
 - **Procedure Arguments**: The `NlmArguments` enum variants (e.g., `Lock(Nlm4LockArgs)`) embed the argument types defined in the `nlm` crate (e.g., `Nlm4LockArgs`), linking the parser to the NLM service.

---

## 4. Data Model

Entities:
- **`RpcHeader`**: Represents the RPC message header.
 - `xid`: `u32` — Transaction ID.
 - `cred`: `OpaqueAuth` — Authentication credentials.
 - `verf`: `OpaqueAuth` — Authentication verifier (currently unused/dead code).
- **`NfsArgWrapper<B>`**: Wrapper for NFSv3 requests.
 - `header`: `RpcHeader` — The RPC header.
 - `proc`: `Box<NfsArguments<B>>` — The specific NFS procedure arguments.
- **`MountArgWrapper`**: Wrapper for MOUNT requests.
 - `header`: `RpcHeader` — The RPC header.
 - `proc`: `Box<MountArguments>` — The specific MOUNT procedure arguments.
- **`NlmArgWrapper`**: Wrapper for NLM requests.
 - `header`: `RpcHeader` — The RPC header.
 - `proc`: `Box<NlmArguments>` — The specific NLM procedure arguments.
- **`ArgWrapper<B>`**: Generic wrapper for any protocol request.
 - `header`: `RpcHeader` — The RPC header.
 - `proc`: `ProcArguments<B>` — The specific procedure arguments.
- **`ErrorWrapper`**: Associates an error with a transaction ID.
 - `xid`: `Option<u32>` — The transaction ID.
 - `error`: `Error` — The error that occurred.
- **`ProcArguments<B>`**: Enum grouping arguments by protocol.
 - `Nfs3(Box<NfsArguments<B>>)` — NFSv3 arguments.
 - `Mount(Box<MountArguments>)` — MOUNT arguments.
 - `Nlm4(Box<NlmArguments>)` — NLM arguments.
- **`NfsArguments<B>`**: Enum of NFSv3 procedure arguments.
 - `Null`, `GetAttr(get_attr::Args)`, `SetAttr(set_attr::Args)`, `LookUp(lookup::Args)`, `Access(access::Args)`, `ReadLink(read_link::Args)`, `Read(read::Args<B>)`, `Write(write::Args<B>)`, `Create(create::Args)`, `MkDir(mk_dir::Args)`, `SymLink(symlink::Args)`, `MkNod(mk_node::Args)`, `Remove(remove::Args)`, `RmDir(rm_dir::Args)`, `Rename(rename::Args)`, `Link(link::Args)`, `ReadDir(read_dir::Args)`, `ReadDirPlus(read_dir_plus::Args)`, `FsStat(fs_stat::Args)`, `FsInfo(fs_info::Args)`, `PathConf(path_conf::Args)`, `Commit(commit::Args)`.
- **`MountArguments`**: Enum of MOUNT procedure arguments.
 - `Null`, `Mount(mnt::Args)`, `Unmount(umnt::Args)`, `Export`, `Dump`, `UnmountAll`.
- **`NlmArguments`**: Enum of NLM procedure arguments.
 - `Null`, `Lock(Nlm4LockArgs)`, `Unlock(Nlm4UnlockArgs)`, `Test(Nlm4TestArgs)`, `Cancel(Nlm4CancelArgs)`.

Relations:
- **Composition**: `NfsArgWrapper`, `MountArgWrapper`, `NlmArgWrapper`, and `ArgWrapper` all compose `RpcHeader` and their respective argument enums.
- **Tagged Union**: `ProcArguments` acts as a tagged union for `NfsArguments`, `MountArguments`, and `NlmArguments`.
- **Association**: `ErrorWrapper` associates an `Error` with an optional `xid`.

Global Invariants:
- The `xid` in `RpcHeader` must match the XID in the RPC message being processed.
- The `verf` field in `RpcHeader` is currently unused but present in the struct definition.

## 5. Error Model

Error Types:
- **`rpc::Error`**: The canonical error type used throughout the RPC layer.

Error Propagation Strategy:
- **Result Alias**: The module defines `pub type Result<T> = std::result::Result<T, Error>`, enforcing the use of `rpc::Error` for all parsing operations.
- **Nested Errors**: The `proc_nested_errors` function provides a mechanism to override a previous error with a new one if a nested operation fails.
- **Error Association**: The `ErrorWrapper` struct allows an error to be associated with a specific `xid` for rejection responses.

Recoverability:
- **Dependent on Error**: Recoverability depends on the specific `rpc::Error` variant returned.

Panics:
- **Allowed**: No.
- **Conditions**: The module defines data structures and utility functions; it does not contain logic that panics.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- None.

---

## 7. Overview

This module is used in order to **define the top-level data structures that represent a fully parsed RPC request**, acting as the bridge between the raw byte stream parsing (handled by sub-modules like `primitive` and `nfsv3`) and the service execution layer (VFS, Mount, NLM).

The system contains a complex, multi-protocol RPC server implementation. It needs to handle NFSv3, MOUNT, and NLM protocols. Each protocol has its own set of procedures and argument types. This module unifies the output of the parsing phase into a set of "Wrapper" structs that include the common RPC header (XID, Auth) and the specific procedure arguments.

A typical usage scenario of the system involves the RPC dispatcher receiving a raw byte stream. It uses the `parser_struct` module (which depends on this module) to parse the stream. The result of that parsing is an `ArgWrapper<B>`. This wrapper contains the `RpcHeader` (identifying the client and transaction) and the `ProcArguments` enum. The dispatcher then matches on the `ProcArguments` enum to determine which service to invoke. If it is `Nfs3`, it extracts the `NfsArguments` (e.g., `Read(read::Args<B>)`) and passes it, along with the header, to the VFS implementation. If it is `Mount`, it extracts `MountArguments` and passes it to the Mount service.

Inside the system, the following things happen and they use this module:
1. **Protocol Aggregation**: The `ProcArguments` enum acts as a central switch that categorizes requests into their respective protocol domains (NFS, MOUNT, NLM). This allows the main server loop to be protocol-agnostic in its routing logic.
2. **Type Safety**: By defining specific argument structs like `NfsArgWrapper<B>` and `MountArgWrapper`, the module ensures that the arguments passed to the service handlers are strongly typed and match the expected signatures of the VFS and Mount traits.
3. **Error Contextualization**: The `ErrorWrapper` struct allows the RPC layer to associate a parsing or dispatch error with the specific `xid` of the request. This is critical for sending correct rejection messages back to the client.

Without this module, the RPC dispatcher would have to manage disjoint types for each protocol's arguments, leading to a fragmented and error-prone interface. This module provides the unified "shape" of a parsed request that the rest of the server relies on.