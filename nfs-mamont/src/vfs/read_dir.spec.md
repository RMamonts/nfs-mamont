<!-- SPEC_HASH: 15c3ab6b3a0cc115a3b906d59a6be8783db55b2c8f62ac5286b4065646d5174e -->
# Module Specification

Module: nfs_mamont::vfs::read_dir
Rust File: src/vfs/read_dir.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::consts::nfsv3`**: Used to import `NFS3_COOKIEVERFSIZE`. This constant defines the fixed size (8 bytes) of the `CookieVerifier` array, ensuring compliance with the NFSv3 protocol specification for directory entry verification.
- **`crate::vfs`**: Used to import the `vfs::Error` enum. This is used within the `Fail` struct to report specific protocol errors, such as `BadCookie`, which indicates that the directory state has changed since the last read operation.
- **`super::file`**: Used to import `file::Handle`, `file::Name`, and `file::Attr`. These types are used to identify the directory being read (`Handle`), name the entries found (`Name`), and return the directory's attributes (`Attr`) in the result structures.
- **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` attribute. This macro generates a `Send` version of the `ReadDir` trait, allowing the trait object to be safely transferred between threads in an asynchronous context (e.g., across Tokio tasks).

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `READDIR` procedure, enabling clients to retrieve directory listings in chunks (pagination) rather than all at once.
- To provide mechanisms for resuming directory reads (`Cookie`) and verifying directory consistency (`CookieVerifier`) to ensure the client's view of the directory remains valid across multiple requests.
- To enforce constraints on the size of data returned (`Args::count`) to prevent network buffer overflows and ensure efficient data transfer.

Inputs:
- **`Args`**: A structure containing:
 - `dir`: A `file::Handle` identifying the directory to read.
 - `cookie`: A `Cookie` (u64) indicating the starting position in the directory. A value of 0 starts from the beginning.
 - `cookie_verifier`: A `CookieVerifier` (8 bytes) used to validate that the directory has not changed since the previous request.
 - `count`: A `u32` representing the maximum byte size of the response (including XDR overhead).

Outputs:
- **`Result<Success, Fail>`**:
 - **`Success`**: Contains the directory attributes, a new `cookie_verifier`, a vector of `Entry` objects, and an `eof` flag indicating if the end of the directory was reached.
 - **`Fail`**: Contains a `vfs::Error` (e.g., `BadCookie`, `IO`) and optionally the directory attributes.

Steps:
1. **Request Reception**: The `read_dir` method is called with `Args`. The client provides the `cookie` from the last entry of the previous response (or 0 for the first request) and the `cookie_verifier` from the previous response.
2. **Validation**: The implementation checks if the provided `cookie_verifier` matches the current state of the directory. If the directory has been modified (e.g., files added/removed) since the verifier was issued, the request is rejected with `Fail` containing `vfs::Error::BadCookie`.
3. **Iteration**: The implementation iterates through directory entries starting at the position indicated by `cookie`.
4. **Size Limiting**: While iterating, the implementation accumulates `Entry` structures. It stops adding entries if the total size of the `Success` structure (including XDR overhead) would exceed `Args::count`.
5. **Response Construction**:
 - If the end of the directory is reached, `eof` is set to `true`.
 - A new `cookie_verifier` is generated (or the current one is retained) and included in the response.
 - The list of entries and the directory's post-operation attributes are packed into `Success`.
6. **Return**: The `Result` is returned to the caller.

Edge Cases:
- **Zero File ID**: The `Entry` struct documentation notes that UNIX clients assign special meaning to `file_id` 0. The server should avoid sending 0, and clients should map it if received.
- **Empty Directory**: If the directory is empty, `entries` will be empty and `eof` will be `true`.
- **Stale Cookie**: If the client sends a `cookie` that does not correspond to a valid entry in the current directory state (e.g., an entry was deleted), the server should return `BadCookie`.

Complexity:
- **Time**: O(N) where N is the number of entries scanned or returned. The complexity depends on the underlying file system implementation.
- **Space**: O(M) where M is the number of entries returned in the `Success` vector, bounded by `Args::count`.

Determinism:
- **Deterministic**: Given a specific directory state and input arguments, the output is deterministic. However, the directory state itself may change non-deterministically due to other operations.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
 - **`vfs::Error`**: This module relies on the `Error` enum defined in the parent VFS module to signal failure conditions. Specifically, the `BadCookie` variant (discriminant 10003) is critical for the `read_dir` logic to inform the client that the directory has changed and the pagination state is invalid.
 - **`Vfs` Trait**: The `ReadDir` trait defined in this module is a component of the larger `Vfs` super-trait. The `Vfs` module aggregates `ReadDir` with other operations, meaning any implementation of `Vfs` must provide a concrete implementation of the `read_dir` method defined here.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle`**: Used in `Args` to uniquely identify the directory being read. The `Handle` acts as an opaque reference passed from the client to the server.
 - **`Name`**: Used in `Entry` to represent the filename of each directory entry. The `Name` type ensures that the filename adheres to protocol constraints (length, valid characters).
 - **`Attr`**: Used in `Success` and `Fail` to return the attributes of the directory itself. This allows the client to update its cache for the directory inode without making a separate `GETATTR` call.

- **From `nfs_mamont::consts::nfsv3`**:
 - **`NFS3_COOKIEVERFSIZE`**: This constant (value 8) is used to define the size of the `CookieVerifier` array. This ensures that the verifier matches the exact wire format size required by the NFSv3 protocol, facilitating correct serialization and deserialization.

---

## 4. Data Model

Entities:
- **`Cookie`**: A wrapper around a `u64` representing a cursor or offset within a directory listing.
 - Invariants: `0` represents the start of the directory.
- **`CookieVerifier`**: A wrapper around an `[u8; 8]` array used to verify the consistency of the directory.
 - Invariants: All zeros (`[0; 8]`) is used for the initial request.
- **`Entry`**: Represents a single item in a directory.
 - Fields: `file_id` (u64), `file_name` (file::Name), `cookie` (Cookie).
 - Invariants: `file_id` should ideally not be 0.
- **`Args`**: Arguments for the `read_dir` operation.
 - Fields: `dir` (file::Handle), `cookie` (Cookie), `cookie_verifier` (CookieVerifier), `count` (u32).
- **`Success`**: Successful result of the operation.
 - Fields: `dir_attr` (Option<file::Attr>), `cookie_verifier` (CookieVerifier), `entries` (Vec<Entry>), `eof` (bool).
- **`Fail`**: Failed result of the operation.
 - Fields: `error` (vfs::Error), `dir_attr` (Option<file::Attr>).

Relations:
- **Composition**: `Args` contains `Cookie` and `CookieVerifier`.
- **Composition**: `Entry` contains `Cookie` and `file::Name`.
- **Composition**: `Success` and `Fail` contain `Option<file::Attr>`.
- **Dependency**: `ReadDir::read_dir` takes `Args` and returns `Result<Success, Fail>`.

Global Invariants:
- **Verifier Size**: `CookieVerifier` is always exactly 8 bytes (`NFS3_COOKIEVERFSIZE`).
- **Cookie Monotonicity**: While not enforced by the type system, the protocol implies that `cookie` values in a stream of entries should generally be monotonically increasing (or at least strictly ordered) to allow the server to find the resume point.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The error type returned within the `Fail` struct. Relevant variants include:
 - `BadCookie`: Indicates the provided cookie or verifier is invalid/stale.
 - `IO`: General input/output error.
 - `NotDir`: The provided handle does not refer to a directory.
 - `StaleFile`: The provided file handle is no longer valid.

Error Propagation Strategy:
- **Result Type**: The method returns `Result<Success, Fail>`. Errors are wrapped in the `Fail` struct, which also includes optional directory attributes. This allows the client to retrieve attributes even if the read fails (e.g., due to permission issues), adhering to the NFSv3 specification.

Recoverability:
- **`BadCookie`**: Recoverable by restarting the directory read from the beginning (cookie = 0, verifier = 0).
- **`IO`**: Potentially recoverable if transient, but often requires user intervention or retry.
- **`StaleFile`**: Recoverable by performing a new `LOOKUP` to obtain a fresh file handle.

Panics:
- **Allowed**: No.
- **Conditions**: This module defines interfaces and data structures; it does not contain logic that should panic. Implementations of the `ReadDir` trait should avoid panicking and return `Fail` instead.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`ReadDir`**: The core trait defining the `read_dir` asynchronous method. It is marked with `#[trait_variant::make(Send)]`, which implies the existence of an auto-generated `ReadDir` trait (object safe) and a `Send` variant for use in async contexts requiring thread safety.

---

## 7. Overview

This section is needed for the evolution of project understanding. 
Because each module is invoked with its own context, you MUST provide enough information about the purpose of the system that incorporates the current module and its dependency on the current module. 
You speak here not only about WHAT the module does. Taking the role of the module’s user, you must say WHY the whole system, consisting of this module and its dependencies, is needed. 
Here MUST formulate a new Overview based on the Overview from the dependency specifications and the analysis of the current module. 
You MUST give an exhaustive description for a new analyzing agent, because the new agent will not be able to look at dependencies deeper than one level. 
You MUST NOT give a simple description of what the system consisting of the module and its dependencies does. You must answer the question — why, because the new agent will not be able to look at dependencies deeper than one level.

This module is used in order to **implement the directory listing capability of the NFSv3 protocol**, specifically handling the complexities of reading large directories over a network protocol with strict size limits. The system contains a high-performance NFS server where the storage backend (VFS) is abstracted from the network handling. Directories on a file system can contain thousands of entries; sending all of them in a single network packet is impossible due to MTU limits and inefficient due to latency.

The `read_dir` module solves this by defining a stateful pagination mechanism. The `Cookie` allows the client to say "give me the next 100 entries starting where I left off," and the `CookieVerifier` ensures that the directory hasn't been modified by another client in the meantime, which would make the "resume point" invalid. This is crucial for data consistency in a concurrent environment.

A typical usage scenario of the system involves a user executing `ls -l` on an NFS client. The client sends a `READDIR` request with `cookie=0`. The server (via the `ReadDir` implementation) returns the first batch of entries and a `cookie` for the last one. The client sends another request with that `cookie`. If, in the meantime, another user deletes a file in that directory, the server detects the change (via the verifier) and returns `BadCookie`, forcing the client to restart the listing. This prevents the client from seeing a "torn" or inconsistent view of the directory.

Inside the system, the following things happen and they use this module:
1.  **Protocol Compliance**: The RPC layer uses the `Args` and `Success` structures defined here to serialize and deserialize requests according to the NFSv3 RFC. The strict typing of `Cookie` and `CookieVerifier` ensures that the 64-bit and 8-bit integer fields are handled correctly.
2.  **VFS Integration**: The `Vfs` trait (defined in the parent module) requires `ReadDir`. This means any storage backend plugged into the server (whether it's a local disk, an S3 bucket, or a memory filesystem) *must* implement this pagination logic. This module enforces that contract.
3.  **Error Handling**: By using `vfs::Error` (specifically `BadCookie`), this module integrates with the server's centralized error reporting system, allowing the RPC layer to convert the error into the correct NFS status code (e.g., `NFS3ERR_BAD_COOKIE`) to be sent back to the client.

Without this module, the server would lack a standardized way to stream directory contents, leading to potential buffer overflows, incomplete listings, or race conditions where clients see inconsistent directory states.