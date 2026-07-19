<!-- SPEC_HASH: e4857555f775038b75647ea17e62c49fb4fadbf3cdfba9a4ab8593d23ecd5284 -->
# Module Specification

Module: nfs_mamont::vfs::rename
Rust File: src/vfs/rename.rs

---

## 1. Dependencies

From *.deps.json, for each dependency, write down its purpose — why it is used in this module.

- **`crate::vfs`**: Used to import the `DirOpArgs` type (for identifying source and target directories and names), `WccData` type (for reporting weak cache consistency data for both directories involved), and the `Error` enum (for reporting specific protocol errors like `XDev` or `Exist`).
- **`trait_variant`**: Used via the `#[trait_variant::make(Send)]` attribute to transform the `Rename` trait into an object-safe trait that also implements `Send`. This allows the trait to be used as a trait object in asynchronous contexts across threads.

---

## 2. Mechanics

Here you need to list the key mechanisms implemented in this module. 
A key mechanism can be identified by its semantic complexity and/or its public visibility.

Intent:
- To define the interface for the NFSv3 `RENAME` procedure.
- To enforce the contract that the rename operation must appear atomic to the client.
- To specify the requirements for handling cross-device renames, target overwriting, and cache consistency updates for both the source and target directories.

Inputs:
- **`args: Args`**: A structure containing:
  - `from`: A `vfs::DirOpArgs` identifying the source directory handle and the current name of the object.
  - `to`: A `vfs::DirOpArgs` identifying the target directory handle and the new name for the object.

Outputs:
- **`Result<Success, Fail>`**:
  - **`Success`**: Contains `from_dir_wcc` and `to_dir_wcc` (Weak Cache Consistency data) indicating the state of the source and target directories before and after the operation.
  - **`Fail`**: Contains a `vfs::Error` describing the failure, plus `from_dir_wcc` and `to_dir_wcc` to allow the client to update its cache even if the operation failed.

Steps:
1. **Validation**: The implementation must verify that the source and target directories reside on the same file system (checking `fsid`). If not, return `vfs::Error::XDev`.
2. **Name Checks**: The implementation must check if the names are "." or "..", or if they are aliases for the directory itself. If so, return `vfs::Error::InvalidArgument`.
3. **Identity Check**: If the source and target arguments refer to the same file (e.g., hard links), the implementation must perform no action and return `Success`.
4. **Target Compatibility**: If the target name already exists in the target directory:
   - If the source and target are incompatible (e.g., one is a directory, the other is not), return `vfs::Error::Exist`.
   - If the target is a non-empty directory, return `vfs::Error::Exist`.
   - If compatible, the existing target must be removed atomically as part of the rename.
5. **Execution**: Perform the rename operation. This must be atomic to the client, though the server may internally use an `unlink/link/unlink` sequence (which might return `vfs::Error::TooManyLinks` if link limits are hit).
6. **WCC Collection**: Collect pre- and post-operation attributes for both the source directory (which lost an entry) and the target directory (which gained or replaced an entry) to populate `WccData`.

Edge Cases:
- **Cross-Device Rename**: Attempting to rename across different file systems results in `vfs::Error::XDev`.
- **Hard Link Rename**: Renaming a file to another name that is a hard link to the same file results in success with no changes.
- **Stale Handles**: The specification notes that file handles *may* become stale, though implementers are encouraged to prevent this.
- **Internal Implementation**: If the server uses `unlink/link/unlink`, it might return `vfs::Error::TooManyLinks` even though the operation is logically atomic.

Complexity:
- **Time**: Dependent on the underlying file system implementation. Typically O(1) for metadata updates, but O(N) if checking if a target directory is empty (where N is the number of entries in the target directory).
- **Space**: O(1) for the argument and result structures.

Determinism:
- **Deterministic**: The behavior is strictly defined by the NFSv3 protocol rules regarding error conditions and atomicity requirements.

---

## 3. Dependency Mechanics

List here the key mechanisms that are important for this module from the Mechanics section of the dependency specifications.

- **From `nfs_mamont::vfs`**:
  - **`DirOpArgs`**: This structure is used to pass the directory handle and entry name for both the source (`from`) and target (`to`) of the rename operation. It standardizes how directory operations are addressed across the VFS layer.
  - **`WccData`**: This structure is critical for the `Success` and `Fail` results. The rename operation modifies two directories (removing from the source, adding to the target). `WccData` allows the server to return the pre- and post-attributes for *both* directories, enabling the client to validate its cache for both locations efficiently.
  - **`Error` Enum**: The module relies on specific variants of this enum to signal protocol-compliant errors, such as `XDev` (cross-device link), `Exist` (target conflict), `InvalidArgument` (illegal names), and `TooManyLinks` (internal implementation limits).

---

## 4. Data Model

Entities:
- **`Args`**: Arguments for the rename operation.
  - `from`: `vfs::DirOpArgs` (Source location).
  - `to`: `vfs::DirOpArgs` (Target location).
- **`Success`**: Result of a successful rename.
  - `from_dir_wcc`: `vfs::WccData` (Cache data for the source directory).
  - `to_dir_wcc`: `vfs::WccData` (Cache data for the target directory).
- **`Fail`**: Result of a failed rename.
  - `error`: `vfs::Error` (The specific error encountered).
  - `from_dir_wcc`: `vfs::WccData` (Cache data for the source directory).
  - `to_dir_wcc`: `vfs::WccData` (Cache data for the target directory).
- **`Rename`**: A trait defining the asynchronous rename operation.

Relations:
- **Composition**: `Args` composes two `vfs::DirOpArgs`.
- **Composition**: `Success` and `Fail` both compose two `vfs::WccData` instances.
- **Association**: `Fail` is associated with a `vfs::Error`.

Global Invariants:
- **Dual WCC**: Both `Success` and `Fail` must always contain `WccData` for both the source and target directories. This ensures the client can synchronize its cache regardless of the operation's outcome.
- **Atomicity**: The operation must appear atomic to the client; intermediate states (like a file existing in both places or neither) must not be observable.

## 5. Error Model

Error Types:
- **`vfs::Error`**: The enumeration used to report failures.
  - `XDev`: Source and target are on different file systems.
  - `TooManyLinks`: Internal link limit reached (if using unlink/link/unlink).
  - `InvalidArgument`: Illegal names (".", "..") or aliasing the directory itself.
  - `Exist`: Target exists and is incompatible (type mismatch or non-empty directory).

Error Propagation Strategy:
- **Custom Enum**: Errors are returned within the `Fail` struct wrapper. The `Fail` struct ensures that even if an error occurs, the `WccData` is propagated back to the caller.

Recoverability:
- **Client-Side**: Most errors (like `Exist` or `InvalidArgument`) indicate client request errors and are not recoverable by retrying the exact same request. `XDev` implies the client must copy data manually. `TooManyLinks` or `IO` errors might be transient or indicate server state issues.

Panics:
- **Allowed**: No. The interface is designed to return all error conditions via `Result<Success, Fail>`.

---

## 6. Traits

List which external traits this module implements:
- None.

List which traits this module defines:
- **`Rename`**: The trait defining the `rename` asynchronous method. It is marked `Send` to allow use across threads.

---

## 7. Overview

This module is used in order to **define the contract for the NFSv3 RENAME procedure** within the Virtual File System (VFS) layer. It encapsulates the specific logic required to move or rename file system objects while adhering to the strict atomicity and consistency requirements of the NFS protocol.

The system contains a complex architecture where file system operations are abstracted behind traits to allow different storage backends. The `rename` module is a specific component of this VFS layer. It is necessary because renaming is a complex operation that affects two directories simultaneously (the source parent and the target parent) and has strict constraints regarding cross-device moves and atomicity.

A typical usage scenario of the system involves an NFS client requesting to move a file from one directory to another. The RPC layer decodes the request and invokes the `rename` method on the VFS implementation. The implementation checks if the move is valid (e.g., same file system), handles the removal of the target if it exists, and performs the rename. Crucially, it gathers `WccData` for both directories to return to the client, ensuring the client's cache remains consistent with the server's state.

Inside the system, the following things happen and they use this module:
1. **Protocol Enforcement**: The module enforces NFSv3 rules, such as returning `XDev` if the client tries to rename across file systems or `Exist` if the target is a non-empty directory. This ensures the server behaves predictably according to the standard.
2. **Cache Consistency Management**: By requiring `WccData` for both the source and target directories in both success and failure cases, the module ensures that the client can update or invalidate its cache entries for both affected directories efficiently, preventing stale data.
3. **Interface Abstraction**: The `Rename` trait allows the upper layers of the server (RPC handling) to remain decoupled from the specific details of how the underlying storage engine performs the rename. The storage engine simply needs to implement this trait to be compatible with the NFS server.