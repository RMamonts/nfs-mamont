<!-- SPEC_HASH: 0b0a77f5fc86f46f794300dc5428d49ff404c4acdf7a36dfbb201c6224a55c39 -->
# Module Specification

Module: nfs_mamont::vfs::read_dir_plus
Rust File: src/vfs/read_dir_plus.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `vfs::Error` enum. This is used within the `Fail` struct to report specific protocol errors (e.g., `BadCookie`, `IO`) that occur during the operation.
- **`crate::vfs::read_dir`**: Used to import the `Cookie` and `CookieVerifier` types. These types are essential for maintaining pagination state and verifying directory consistency, shared with the standard `READDIR` procedure.
- **`super::file`**: Used to import `file::Handle`, `file::Name`, and `file::Attr`. These types are used to identify the directory being read (`Handle`), name the entries (`Name`), and return the full metadata and handles for each entry (`Attr`, `Handle`) within the `Entry` struct.
- **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` attribute. This macro generates a `Send` version of the `ReadDirPlus` trait, allowing the trait object to be safely transferred between threads in an asynchronous context.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `READDIRPLUS` procedure. This procedure extends the standard directory listing (`READDIR`) by returning file attributes and file handles for each entry in the same response.
- To optimize client performance by eliminating the need for subsequent `LOOKUP` RPC calls for every directory entry to retrieve attributes or handles.
- To enforce strict size constraints on the response data using two distinct limits (`dir_count` and `max_count`) to manage memory usage and network packet fragmentation.

Inputs:
- **`Args`**: A structure containing:
 - `dir`: A `file::Handle` identifying the directory to read.
 - `cookie`: A `Cookie` (u64) indicating the starting position. `0` starts from the beginning.
 - `cookie_verifier`: A `CookieVerifier` used to validate that the directory has not changed since the previous request.
 - `dir_count`: A `u32` specifying the maximum bytes for directory information (names, cookies, file IDs) *excluding* attributes and handles.
 - `max_count`: A `u32` specifying the maximum total size of the `Success` structure, including XDR overhead, attributes, and handles.

Outputs:
- **`Result<Success, Fail>`**:
 - **`Success`**: Contains the directory attributes, a new `cookie_verifier`, a vector of `Entry` objects (which include attributes and handles), and an `eof` flag.
 - **`Fail`**: Contains a `vfs::Error` and optionally the directory attributes.

Steps:
1. **Request Reception**: The `read_dir_plus` method is called with `Args`. The client provides the `cookie` and `cookie_verifier` from the previous response (or zeros for the first request).
2. **Validation**: The implementation verifies the `cookie_verifier` against the current directory state. If the directory has been modified, the request is rejected with `Fail` containing `vfs::Error::BadCookie`.
3. **Iteration and Accumulation**: The implementation iterates through directory entries starting at the `cookie`.
4. **Size Enforcement**:
 - The implementation accumulates entry data while respecting `dir_count` (limiting the "pure" directory data) and `max_count` (limiting the total response size).
 - Since `Entry` includes optional `file_attr` and `file_handle`, the implementation must calculate the size of these fields to ensure the total does not exceed `max_count`.
5. **Response Construction**:
 - If the end of the directory is reached, `eof` is set to `true`.
 - A new `cookie_verifier` is generated/retained.
 - The list of entries and the directory's post-operation attributes are packed into `Success`.
6. **Return**: The `Result` is returned to the caller.

Edge Cases:
- **Zero File ID**: The `Entry` struct documentation explicitly warns that UNIX clients assign special meaning to `file_id` 0. The server should avoid sending 0, and clients should map it if received.
- **Optional Attributes/Handles**: The `Entry` struct defines `file_attr` and `file_handle` as `Option`. This allows the server to omit them if they cannot be retrieved or if including them would violate `max_count` constraints, though the protocol intent is to provide them.
- **Empty Directory**: If the directory is empty, `entries` will be empty and `eof` will be `true`.

Complexity:
- **Time**: O(N) where N is the number of entries scanned or returned. Fetching attributes for each entry may add overhead compared to `READDIR`.
- **Space**: O(M) where M is the number of entries returned, bounded by `max_count`.

Determinism:
- **Deterministic**: Given a specific directory state and input arguments, the output is deterministic.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs::read_dir`**:
 - **`Cookie` and `CookieVerifier`**: This module reuses the pagination and state verification mechanisms defined in `read_dir`. The logic for resuming a read (`cookie`) and ensuring the directory hasn't changed (`cookie_verifier`) is identical to the base `READDIR` procedure.
 - **`ReadDir` Trait**: `ReadDirPlus` is a semantic superset of `ReadDir`. While they are separate traits, the underlying mechanics of traversing the directory structure are shared.

- **From `nfs_mamont::vfs::file`**:
 - **`Handle` and `Attr`**: Unlike `read_dir::Entry`, `read_dir_plus::Entry` includes these types. This shifts the burden of attribute lookup from the client (via subsequent `LOOKUP` calls) to the server (during the `READDIRPLUS` call).
 - **`Name`**: Used identically to `read_dir` to represent the filename of the entry.

- **From `nfs_mamont::vfs`**:
 - **`vfs::Error`**: The error handling model is consistent with the rest of the VFS layer. `BadCookie` is particularly relevant here, as a change in directory contents invalidates not just the position but potentially any cached attributes the client might hold.

---

## 4. Data Model

Entities:
- **`Entry`**: Represents a single item in a directory with extended information.
 - Fields: `file_id` (u64), `file_name` (file::Name), `cookie` (Cookie), `file_attr` (Option<file::Attr>), `file_handle` (Option<file::Handle>).
 - Invariants: `file_id` should ideally not be 0.
- **`Success`**: Successful result of the operation.
 - Fields: `dir_attr` (Option<file::Attr>), `cookie_verifier` (CookieVerifier), `entries` (Vec<Entry>), `eof` (bool).
- **`Fail`**: Failed result of the operation.
 - Fields: `error` (vfs::Error), `dir_attr` (Option<file::Attr>).
- **`Args`**: Arguments for the `read_dir_plus` operation.
 - Fields: `dir` (file::Handle), `cookie` (Cookie), `cookie_verifier` (CookieVerifier), `dir_count` (u32), `max_count` (u32).

Relations:
- **Composition**: `Entry` contains `file::Attr` and `file::Handle` (optionally), extending the basic `read_dir::Entry` structure.
- **Composition**: `Success` and `Fail` contain `Option<file::Attr>` for the directory itself.
- **Dependency**: `ReadDirPlus::read_dir_plus` takes `Args` and returns `Result<Success, Fail>`.

Global Invariants:
- **Size Limits**: The response must adhere to both `dir_count` (limiting name/cookie data) and `max_count` (limiting total response size).
- **Verifier Consistency**: The `cookie_verifier` returned in `Success` must be used by the client in the subsequent request to ensure directory consistency.

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
- **Conditions**: This module defines interfaces and data structures; it does not contain logic that should panic. Implementations of the `ReadDirPlus` trait should avoid panicking and return `Fail` instead.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`ReadDirPlus`**: The core trait defining the `read_dir_plus` asynchronous method. It is marked with `#[trait_variant::make(Send)]`, which implies the existence of an auto-generated `ReadDirPlus` trait (object safe) and a `Send` variant for use in async contexts requiring thread safety.

---

## 7. Overview

This module is used in order to **implement the enhanced directory listing capability of the NFSv3 protocol**, specifically designed to reduce the number of Remote Procedure Calls (RPCs) required for a client to display a detailed directory view. The system contains a high-performance NFS server where the storage backend (VFS) is abstracted from the network handling. In a standard `READDIR` operation, the server returns only filenames and file IDs. To display details like file size, permissions, or owner (as in `ls -l`), the client would traditionally have to issue a separate `LOOKUP` RPC for every single file in the directory to get its attributes and handle. This "N^1" problem is inefficient over high-latency networks.

The `read_dir_plus` module solves this by defining an interface that returns the file attributes (`file::Attr`) and file handles (`file::Handle`) inline with the directory entries. This allows the client to populate a detailed file list in a single request/response round-trip.

A typical usage scenario of the system involves a user executing `ls -l` on an NFS client. The client sends a `READDIRPLUS` request. The server (via the `ReadDirPlus` implementation) iterates the directory, looks up the attributes for each entry, and packs them all into the `Success` structure. The client receives the names, sizes, modes, and handles immediately, without further network communication.

Inside the system, the following things happen and they use this module:
1.  **Protocol Optimization**: The RPC layer uses the `Args` and `Success` structures defined here to serialize requests and responses. The presence of `dir_count` and `max_count` in `Args` allows the client to fine-tune the request: `dir_count` ensures it gets enough names, while `max_count` ensures the total packet size (including potentially large attribute sets) doesn't exceed the transport limit.
2.  **VFS Integration**: The `Vfs` trait (defined in the parent module) requires `ReadDirPlus`. This ensures that any storage backend plugged into the server is capable of providing this optimized lookup path.
3.  **State Management**: Like `read_dir`, this module relies on `Cookie` and `CookieVerifier` to handle large directories that span multiple responses. If the directory changes mid-read, the `BadCookie` error ensures the client detects the inconsistency and restarts, preventing the display of corrupted or partial file information.

Without this module, the server would force clients to perform expensive `LOOKUP` loops for directory listings, significantly degrading performance for directories with many files. This module is critical for providing a responsive and efficient user experience over NFS.