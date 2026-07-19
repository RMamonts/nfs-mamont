<!-- SPEC_HASH: 8f66b5273513df71959c2d8776d064196c05ca8dbef2128520a5a79c84e374e3 -->
# Module Specification

Module: nfs_mamont::vfs::remove
Rust File: src/vfs/remove.rs

---

## 1. Dependencies

- **`crate::vfs`**
  - **Purpose:** This module acts as a sub-module of the VFS layer. It depends on the parent `vfs` module for core type definitions that are shared across different file system operations.
  - **Specific Usage:**
    - `vfs::DirOpArgs`: Used in `Args` to identify the target directory and the specific entry name to be removed.
    - `vfs::WccData`: Used in `Success` and `Fail` to provide Weak Cache Consistency data (attributes of the directory before and/or after the operation) to the client, ensuring cache coherence in the NFSv3 protocol.
    - `vfs::Error`: Used in `Fail` to report specific protocol-level errors (e.g., `NoEntry`, `Access`, `NotDir`) that occurred during the removal attempt.

---

## 2. Mechanics

**Intent:**
To define the asynchronous interface for removing (deleting) a file system object from a directory, strictly adhering to the NFSv3 protocol requirements regarding error reporting and cache consistency.

**Inputs:**
- `args: Args`: A structure containing `object` of type `vfs::DirOpArgs`. This encapsulates the handle of the directory (`dir`) and the name of the entry (`name`) to be removed.

**Outputs:**
- `Result<Success, Fail>`:
  - `Ok(Success)`: Indicates the entry was successfully removed. Contains `wcc_data` (Weak Cache Consistency data) for the directory, allowing the client to update its cache with the post-operation state.
  - `Err(Fail)`: Indicates the operation failed. Contains the specific `vfs::Error` and `dir_wcc` (Weak Cache Consistency data for the directory). The inclusion of WCC data on failure allows the client to synchronize its cache even if the operation did not succeed.

**Steps:**
1. The consumer calls the `remove` method with `Args` identifying the target directory and entry name.
2. The implementation attempts to delete the entry from the specified directory.
3. **On Success:**
   - The entry is removed.
   - The implementation captures the directory attributes *after* the modification.
   - Returns `Success` with `wcc_data` populated (presumably with `after` attributes).
4. **On Failure:**
   - The entry remains (or the state is unchanged).
   - The implementation captures the directory attributes *before* the operation (or current attributes).
   - Returns `Fail` containing the specific `vfs::Error` and `dir_wcc`.

**Edge Cases:**
- **Non-existent entry:** The interface expects the implementation to return a `Fail` with `vfs::Error::NoEntry`.
- **Permission denied:** The interface expects the implementation to return a `Fail` with `vfs::Error::Permission` or `vfs::Error::Access`.
- **Directory removal:** The interface is named `Remove` (NFSv3 `REMOVE`), which typically applies to non-directory files. If a directory is passed, the implementation should likely return `vfs::Error::IsDir` or `vfs::Error::NotDir` (depending on specific NFS server semantics, though `REMOVE` is usually for files, `RMDIR` is for directories).
- **Non-empty directory:** If applied to a directory (if supported/allowed by the specific implementation logic), `vfs::Error::NotEmpty` might be relevant, though standard NFSv3 `REMOVE` on a directory usually fails with `IsDir`.

**Complexity:**
- **Time:** Unspecified by this interface; depends on the underlying file system implementation.
- **Space:** Unspecified by this interface; depends on the underlying file system implementation.

**Determinism:**
- **Deterministic:** The interface signature is deterministic. The result depends entirely on the state of the file system identified by `vfs::DirOpArgs`.

---

## 3. Dependency Mechanics

From `nfs_mamont::vfs` (parent module):

- **`vfs::DirOpArgs`**:
  - **Mechanism:** Acts as a composite key locating a specific file system entry.
  - **Relevance:** The `Remove` operation requires the context of the parent directory (`dir`) and the specific filename (`name`) to perform the unlink operation.

- **`vfs::WccData`**:
  - **Mechanism:** Encapsulates `before` and `after` attributes of a directory.
  - **Relevance:** NFSv3 requires the server to return attributes to help the client validate cache. `Success` returns the state *after* removal, while `Fail` returns the state *before* (or current) to ensure the client's cache remains consistent even if the operation fails.

- **`vfs::Error`**:
  - **Mechanism:** Enumerates standard NFSv3 protocol errors.
  - **Relevance:** Provides a typed way to signal failure reasons (e.g., `NoEntry`, `Access`) back to the NFS client.

---

## 4. Data Model

**Entities:**
- **`Args`**
  - Represents the input payload for the removal operation.
  - Field `object`: `vfs::DirOpArgs` (Directory handle + Entry name).
- **`Success`**
  - Represents the successful result of a removal.
  - Field `wcc_data`: `vfs::WccData` (Post-operation directory attributes).
- **`Fail`**
  - Represents the failed result of a removal.
  - Field `error`: `vfs::Error` (The specific error code).
  - Field `dir_wcc`: `vfs::WccData` (Directory attributes, likely pre-operation).

**Relations:**
- `Args` → `vfs::DirOpArgs` (Composition)
- `Success` → `vfs::WccData` (Composition)
- `Fail` → `vfs::Error` (Composition)
- `Fail` → `vfs::WccData` (Composition)

**Global Invariants:**
- None defined in this module (interface definition only).

---

## 5. Error Model

**Error Types:**
- **`vfs::Error`**: An enumeration covering standard NFSv3 errors (e.g., `NoEntry`, `Access`, `IO`, `NotDir`, `IsDir`, `Permission`).

**Error Propagation Strategy:**
- **Custom Result Wrapper:** The trait returns `Result<Success, Fail>`. Errors are not thrown as exceptions or returned as a simple `Err(Error)`. Instead, they are wrapped in the `Fail` struct, which bundles the error with `dir_wcc` (Weak Cache Consistency data). This ensures that cache synchronization data is always available to the caller, regardless of success or failure.

**Recoverability:**
- Recoverable. The interface explicitly separates success and failure paths, allowing the caller (NFS server handler) to construct the appropriate NFSv3 response packet (either `REMOVE3resok` or `REMOVE3resfail`).

**Panics:**
- **Allowed:** No.
- **Conditions:** As a trait definition for a VFS, implementations are expected to return `Err(Fail)` for all error conditions rather than panicking.

---

## 6. Traits

**Defined:**
- **`Remove`**: An asynchronous trait (`async fn remove`) marked with `#[trait_variant::make(Send)]`. This indicates that the trait is object-safe (or made so via the macro) and can be used as a `dyn` trait object in a multi-threaded context (`Send`).

**Implemented:**
- None in this file.

---

## 7. Overview

This module is used in order to define the contract for the NFSv3 `REMOVE` procedure within the Virtual File System (VFS) abstraction layer. It separates the definition of the removal operation from its concrete implementation, allowing different underlying storage backends to be plugged into the NFS server.

This system contains a set of VFS operation traits (like `Lookup`, `Create`, `Remove`) that mirror the NFSv3 protocol procedures. The `vfs::remove` module specifically handles the semantics of deleting a file system object. It is distinct from `RmDir` (directory removal) and focuses on files or other non-directory entries.

A typical usage scenario of the system involves an NFS server receiving a `REMOVE` request from a client. The server handler locates the target directory using the file handle provided in the request, then invokes the `remove` method on the VFS implementation. The VFS implementation performs the file system specific unlink operation. Crucially, because NFS is a stateless protocol (mostly), the client relies on Weak Cache Consistency (WCC) data to validate its local cache. This module enforces that the implementation must return this WCC data (directory attributes) in both `Success` and `Fail` cases.

Inside the system the following things happen and they use the `vfs::remove` interface:
1. The server translates the RPC arguments into `Args` (specifically `vfs::DirOpArgs`).
2. The `remove` trait method is awaited.
3. The result is unpacked: if `Success`, the server sends the post-op attributes; if `Fail`, the server sends the error code and the pre-op attributes. This ensures the client can detect if the directory was modified by another client in the interim, maintaining cache coherence without explicit locking.