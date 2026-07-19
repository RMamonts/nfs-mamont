<!-- SPEC_HASH: 679d76ca02e3723470563d85f21cacf418d8295a949decc8f2d264baac73d7ce -->
# Module Specification

Module: nfs_mamont::parser
Rust File: src/parser/mod.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::allocator::Buffer`**: Used as a generic type parameter `B` in `NfsArgWrapper`, `ArgWrapper`, and `NfsArguments`. This allows the parser to return arguments that contain data buffers (e.g., for `WRITE` operations) managed by the server's custom allocator, enabling zero-copy or controlled-copy semantics.
- **`crate::mount::{mnt, umnt}`**: Used to import the argument types (`mnt::Args`, `umnt::Args`) for the MOUNT protocol procedures. These are embedded in the `MountArguments` enum to provide a unified interface for MOUNT requests.
- **`crate::nlm::procedures`**: Used to import argument types (`Nlm4LockArgs`, `Nlm4UnlockArgs`, `Nlm4TestArgs`, `Nlm4CancelArgs`) for the NLMv4 protocol procedures. These are embedded in the `NlmArguments` enum.
- **`crate::rpc::{Error, OpaqueAuth}`**: Used to define the `Result` type alias and to provide the `OpaqueAuth` type for the `RpcHeader`. `Error` is the canonical error type for parsing failures.
- **`crate::vfs::{...}`**: Used to import argument types for all NFSv3 procedures (e.g., `get_attr::Args`, `read::Args`, `write::Args`, etc.). These are embedded in the `NfsArguments` enum to directly map parsed wire data to VFS trait inputs.
- **`std::future::Future`**: Used in the `proc_nested_errors` helper function to handle asynchronous error propagation.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To provide a unified type system for the results of RPC message parsing, bridging the gap between the raw network stream and the high-level service traits (VFS, MOUNT, NLM).
- To group parsed procedure arguments by protocol (NFS, MOUNT, NLM) to facilitate generic dispatching logic in the RPC layer.
- To encapsulate the RPC header (metadata) alongside the parsed procedure arguments, ensuring that context like the transaction ID (XID) and authentication credentials is preserved throughout the request lifecycle.

Inputs:
- Parsed data structures from submodules (e.g., `vfs::read::Args` from the NFS parser, `mnt::Args` from the MOUNT parser).
- `error: Error`: An error instance used in `proc_nested_errors` to represent the "parent" context error.

Outputs:
- `Result<T>`: A type alias for `std::result::Result<T, Error>`.
- `RpcHeader`: A structure containing the RPC message metadata (`xid`, `cred`, `verf`).
- `NfsArgWrapper<B>`, `MountArgWrapper`, `NlmArgWrapper`: Protocol-specific wrappers combining the `RpcHeader` with the specific procedure arguments.
- `ArgWrapper<B>`: A generic wrapper combining the `RpcHeader` with a `ProcArguments` enum.
- `ErrorWrapper`: A structure associating an `Error` with an optional `xid`.
- `ProcArguments<B>`: An enum grouping arguments by top-level RPC program (NFS, MOUNT, NLM).
- `NfsArguments<B>`, `MountArguments`, `NlmArguments`: Enums enumerating the specific procedures and their arguments for each protocol.

Steps:
1. **Header Definition**: The `RpcHeader` struct is defined to hold the transaction ID (`xid`), credentials (`cred`), and verifier (`verf`). The `verf` field is currently marked as dead code, indicating it is parsed but not yet utilized in authentication logic.
2. **Wrapper Definition**: Wrapper structs (`NfsArgWrapper`, `MountArgWrapper`, `NlmArgWrapper`, `ArgWrapper`) are defined to aggregate the `RpcHeader` with the specific procedure arguments. This allows the service layer to access both the transport context and the operation payload in a single object.
3. **Enum Definition**: The `ProcArguments` enum is defined to discriminate between the three main protocols supported by the server (NFSv3, MOUNT, NLMv4). Each variant holds a boxed set of arguments for that protocol.
4. **Procedure Enumeration**: The `NfsArguments`, `MountArguments`, and `NlmArguments` enums are defined to list all supported procedures for their respective protocols. Each variant holds the specific argument struct (e.g., `NfsArguments::Read` holds `read::Args`).
5. **Error Handling**: The `proc_nested_errors` function is defined to handle scenarios where a parsing operation is nested (e.g., parsing an optional field). It awaits a future; if the future returns `Ok`, it discards the result and returns the provided `error`. If the future returns `Err`, it returns that new error.

Edge Cases:
- **Unused Verifier**: The `verf` field in `RpcHeader` is marked with `#[allow(dead_code)]`, indicating that while it is parsed from the wire, it is not currently used for verification logic in the server.
- **Nested Errors**: The `proc_nested_errors` function implies a specific error handling strategy where a failure in a nested context (like an optional field) overrides the parent context error.

Complexity:
- Time: O(1) for struct construction and enum instantiation. The complexity is dominated by the parsing logic in the submodules that generate the input arguments.
- Space: O(N) where N is the size of the specific argument structs (e.g., the size of a directory listing vector in `read_dir::Args`).

Determinism:
- Deterministic (The data structures are pure definitions; the behavior of `proc_nested_errors` is deterministic based on the input future).

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::rpc`**:
 - **`Error`**: The canonical error type for the parser. This module aliases `Result<T>` to `std::result::Result<T, Error>`, ensuring that all parsing failures are reported using this specific error type.
 - **`OpaqueAuth`**: Used in `RpcHeader` to represent the authentication credentials parsed from the RPC message header.

- **From `nfs_mamont::vfs` (submodules)**:
 - **Argument Structs**: The module relies on the specific argument structs defined in the VFS submodules (e.g., `vfs::read::Args`, `vfs::write::Args`). These structs are directly embedded in the `NfsArguments` enum variants. This creates a direct mapping from the parsed wire format to the input required by the VFS trait implementations.

- **From `nfs_mamont::nlm::procedures` (submodules)**:
 - **Argument Structs**: The module relies on the specific argument structs defined in the NLM procedure submodules (e.g., `Nlm4LockArgs`). These are embedded in the `NlmArguments` enum variants to map parsed NLM requests to the NLM service trait inputs.

- **From `nfs_mamont::mount` (submodules)**:
 - **Argument Structs**: The module relies on `mnt::Args` and `umnt::Args` to represent MOUNT protocol requests, embedded in `MountArguments`.

- **From `nfs_mamont::allocator`**:
 - **`Buffer`**: The `NfsArguments::Write` variant and the `ArgWrapper` struct are generic over `B: Buffer`. This allows the parser to return arguments that include references to memory buffers managed by the server's allocator, which is essential for high-performance I/O operations like `WRITE`.

---

## 4. Data Model

Entities:
- **`RpcHeader`**: Represents the RPC request header.
 - `xid`: `u32` - Transaction ID.
 - `cred`: `OpaqueAuth` - Caller credentials.
 - `verf`: `OpaqueAuth` - Verifier (currently unused).
- **`NfsArgWrapper<B: Buffer>`**: Wrapper for NFS procedure arguments.
 - `header`: `RpcHeader` - The RPC header.
 - `proc`: `Box<NfsArguments<B>>` - The specific NFS procedure arguments.
- **`MountArgWrapper`**: Wrapper for MOUNT protocol arguments.
 - `header`: `RpcHeader` - The RPC header.
 - `proc`: `Box<MountArguments>` - The specific MOUNT procedure arguments.
- **`NlmArgWrapper`**: Wrapper for NLM protocol arguments.
 - `header`: `RpcHeader` - The RPC header.
 - `proc`: `Box<NlmArguments>` - The specific NLM procedure arguments.
- **`ArgWrapper<B: Buffer>`**: Generic wrapper for RPC arguments.
 - `header`: `RpcHeader` - The RPC header.
 - `proc`: `ProcArguments<B>` - The procedure arguments grouped by protocol.
- **`ErrorWrapper`**: Wrapper for errors associated with an RPC transaction.
 - `xid`: `Option<u32>` - The transaction ID, if available.
 - `error`: `Error` - The parsing error.
- **`ProcArguments<B: Buffer>`**: Enum grouping arguments by top-level RPC program.
 - `Nfs3(Box<NfsArguments<B>>`) - NFSv3 arguments.
 - `Mount(Box<MountArguments>)` - MOUNT protocol arguments.
 - `Nlm4(Box<NlmArguments>)` - NLMv4 arguments.
- **`NfsArguments<B: Buffer>`**: Enum enumerating NFSv3 procedure arguments.
 - Variants for all NFSv3 procedures (e.g., `GetAttr`, `SetAttr`, `Lookup`, `Read`, `Write`, etc.), holding their respective `Args` structs.
- **`MountArguments`**: Enum enumerating MOUNT protocol procedure arguments.
 - `Null`, `Mount(mnt::Args)`, `Unmount(umnt::Args)`, `Export`, `Dump`, `UnmountAll`.
- **`NlmArguments`**: Enum enumerating NLMv4 procedure arguments.
 - `Null`, `Lock(Nlm4LockArgs)`, `Unlock(Nlm4UnlockArgs)`, `Test(Nlm4TestArgs)`, `Cancel(Nlm4CancelArgs)`.

Relations:
- **Composition**: `NfsArgWrapper`, `MountArgWrapper`, `NlmArgWrapper`, and `ArgWrapper` all contain `RpcHeader`.
- **Composition**: `ProcArguments` contains `NfsArguments`, `MountArguments`, or `NlmArguments`.
- **Association**: `ErrorWrapper` associates an `Error` with an optional `xid`.

Global Invariants:
- **Header Consistency**: The `header` field in all wrapper structs must match the RPC header of the message being processed.
- **Protocol Mapping**: The `ProcArguments` enum variant must correspond to the RPC program number parsed from the message.
- **Buffer Generic**: The `B: Buffer` generic in `NfsArgWrapper` and `ArgWrapper` implies that any buffer type implementing the `Buffer` trait can be used, though in practice it is the server's custom allocator.

## 5. Error Model

Error Types:
- **`Error`**: The error type defined in `crate::rpc` (re-exported here). This covers I/O errors, parsing errors (invalid discriminants, bad strings), and protocol errors (version mismatch, auth failure).

Error Propagation Strategy:
- **Type Alias**: The module uses `pub type Result<T> = std::result::Result<T, Error>`. This enforces that all public functions returning a result use the `rpc::Error` type.
- **Nested Errors**: The `proc_nested_errors` function provides a specific strategy for handling errors in nested parsing contexts (e.g., optional fields). It prioritizes the error returned by the nested future over the provided parent error.

Recoverability:
- **Unrecoverable for Message**: If parsing fails (returns `Err`), the RPC message is typically considered malformed and cannot be processed further. The connection may be closed or an error response sent.
- **Nested Context**: In `proc_nested_errors`, if the nested future fails, that error is returned; otherwise, the parent error is returned. This allows for specific recovery strategies in optional field parsing (e.g., "if the optional field is garbage, use the parent error").

Panics:
- Allowed: No
- Conditions: This module defines data structures and a helper function; it does not contain logic that triggers panics.

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

This module is used in order to define the canonical data structures that bridge the network parsing layer and the service execution layer in the `nfs_mamont` NFS server. It acts as the central aggregation point for all parsed RPC requests, translating raw bytes into strongly-typed Rust structs that the server's core logic can consume.

The system contains a complex parser hierarchy where low-level modules (`primitive`, `rpc`) handle the byte-level XDR decoding, mid-level modules (`nfsv3`, `nlm`, `mount`) handle the protocol-specific argument structures, and this module (`parser::mod`) defines the high-level containers that unify these results. A typical usage scenario of the system involves the RPC dispatcher receiving a message. The dispatcher first parses the generic RPC header to identify the program and procedure. It then invokes the specific parser for that protocol (e.g., `parser::nfsv3`). The specific parser returns a `ProcArguments` enum variant defined in this module, containing the detailed arguments (e.g., `NfsArguments::Read`). The dispatcher then wraps this in an `ArgWrapper` along with the original `RpcHeader` and passes it to the service handler.

Inside the system, the following things happen and they use this module:
- **Type Safety & Dispatch**: The `ProcArguments` enum allows the main server loop to pattern match on the protocol type (NFS, MOUNT, NLM) without knowing the specific details of every procedure. It simply passes the `Box<NfsArguments>` to the VFS handler, which then matches on the specific procedure (e.g., `Read`, `Write`).
- **Context Preservation**: By including `RpcHeader` in the wrapper structs, the system ensures that the transaction ID (`xid`) and authentication credentials (`cred`) are available to the service layer. This is critical for sending correct responses and for logging/auditing.
- **Memory Management**: The generic `B: Buffer` parameter in `NfsArgWrapper` and `ArgWrapper` allows the parser to return arguments that contain references to memory buffers allocated by the `nfs_mamont::allocator`. This is essential for the `WRITE` procedure, where the data payload must be written directly into a pre-allocated buffer to avoid extra copies.

Without this module, the parser subsystem would lack a unified interface. The RPC dispatcher would have to manage disjoint types for every protocol and procedure, leading to code duplication and a lack of type safety. This module enforces a strict contract: "a parsed request is a Header + a set of Arguments," which is the fundamental abstraction required to drive the asynchronous, multi-protocol server.