<!-- SPEC_HASH: a571d25cc4dcd8906d5068ee8d320b1755db21d21e4f37a8a6a56083603050db -->
# Module Specification

Module: nfs_mamont::vfs::rm_dir
Rust File: src/vfs/rm_dir.rs

---

## 1. Dependencies

- **`crate::vfs`**
  - **Purpose:** Provides the core data structures used to define the inputs and outputs of the `RmDir` operation.
  - **Specifics:**
    - `vfs::DirOpArgs`: Used in `Args` to identify the target directory (via a file handle) and the specific name of the subdirectory to be removed.
    - `vfs::WccData`: Used in `Success` and `Fail` to carry Weak Cache Consistency data (attributes before and after the operation) for the parent directory, allowing clients to validate their cache.
    - `vfs::Error`: Used in `Fail` to indicate the specific reason for the operation's failure (e.g., `NoEntry`, `NotEmpty`, `Permission`).

- **`trait_variant`**
  - **Purpose:** Used to generate a `Send`-compatible version of the `RmDir` trait.
  - **Specifics:** The `#[trait_variant::make(Send)]` attribute allows the trait to be used as a trait object in asynchronous contexts that require thread safety (`dyn RmDir + Send`).

---

## 2. Mechanics

**Intent:**
To define the interface for the NFSv3 `RMDIR` procedure. This interface abstracts the operation of removing a subdirectory from a parent directory, enforcing specific NFSv3 semantics regarding special filenames (".", "..") and ensuring that cache consistency data is returned to the client.

**Inputs:**
- `args: Args`: A structure containing `object`, which is a `vfs::DirOpArgs`. This encapsulates the file handle of the parent directory and the name of the directory entry to remove.

**Outputs:**
- `Result<Success, Fail>`:
  - `Ok(Success)`: Indicates successful removal. Contains `wcc_data` (post-operation attributes).
  - `Err(Fail)`: Indicates failure. Contains `error` (the specific `vfs::Error`) and `dir_wcc` (pre-operation attributes of the directory).

**Steps:**
1. The implementation receives the `Args`, identifying the parent directory handle and the target name.
2. The implementation checks the target name against NFSv3 constraints:
   - If the name is ".", the operation fails with `vfs::Error::InvalidArgument`.
   - If the name is "..", the operation fails with `vfs::Error::Exist`.
3. The implementation attempts to remove the directory entry from the underlying storage.
4. If successful, it returns `Success` with `WccData` reflecting the state of the parent directory after the modification.
5. If unsuccessful, it returns `Fail` with the specific `vfs::Error` and `WccData` reflecting the state of the parent directory before the failed attempt.

**Edge Cases:**
- **Special Filenames:** The interface explicitly mandates that attempts to remove "." or ".." result in specific errors (`InvalidArgument` and `Exist` respectively), mirroring NFSv3 server behaviors.
- **Non-empty Directories:** While not explicitly detailed in this trait's docstring, standard NFS behavior implies that removing a non-empty directory should result in `vfs::Error::NotEmpty` (defined in `vfs::Error`).

**Complexity:**
- **Time:** Unspecified by this trait; depends on the underlying file system implementation.
- **Space:** Unspecified by this trait; depends on the underlying file system implementation.

**Determinism:**
- **Deterministic:** The behavior is strictly defined by the NFSv3 specification and the input arguments.

---

## 3. Dependency Mechanics

Since specifications for dependencies are not provided, the following mechanics are inferred from the `*.facts.json` files of the dependencies:

- **`vfs::DirOpArgs` (from `nfs_mamont::vfs`)**:
  - Acts as the carrier for the operation context, holding the `file::Handle` of the parent directory and the `file::Name` of the entry to be removed.
- **`vfs::WccData` (from `nfs_mamont::vfs`)**:
  - Encapsulates the mechanism for Weak Cache Consistency. It holds optional `file::WccAttr` (before) and `file::Attr` (after). This is critical for the `RmDir` interface to return sufficient information for the client to synchronize its cache.
- **`vfs::Error` (from `nfs_mamont::vfs`)**:
  - Provides a comprehensive enumeration of failure modes (e.g., `Permission`, `NoEntry`, `NotEmpty`, `StaleFile`). The `RmDir` interface relies on this enum to communicate specific failure reasons to the caller.

---

## 4. Data Model

**Entities:**
- **`Args`**: A wrapper structure holding the necessary context to perform the removal.
  - `object: vfs::DirOpArgs`: Identifies the directory and the entry name.
- **`Success`**: Represents a successful operation outcome.
  - `wcc_data: vfs::WccData`: Post-operation attributes of the parent directory.
- **`Fail`**: Represents a failed operation outcome.
  - `error: vfs::Error`: The specific error encountered.
  - `dir_wcc: vfs::WccData`: Pre-operation attributes of the parent directory.

**Relations:**
- `Args` **contains** `vfs::DirOpArgs` (1:1).
- `Success` **contains** `vfs::WccData` (1:1).
- `Fail` **contains** `vfs::Error` (1:1) and `vfs::WccData` (1:1).

**Global Invariants:**
- If the operation returns `Fail`, the `dir_wcc` must reflect the state of the directory *before* the attempted removal.
- If the operation returns `Success`, the `wcc_data` must reflect the state of the directory *after* the removal.

---

## 5. Error Model

**Error Types:**
- **`vfs::Error`**: An enumeration covering all standard NFSv3 error codes (e.g., `NoEntry`, `NotEmpty`, `IO`, `StaleFile`).

**Error Propagation Strategy:**
- **Custom Result Type**: The trait uses `Result<Success, Fail>`. Errors are not thrown as exceptions or returned as a simple `Err(Error)`. Instead, they are wrapped in the `Fail` struct, which bundles the error with cache consistency data (`dir_wcc`). This ensures the client receives the necessary metadata to update its cache even when the operation fails.

**Recoverability:**
- The interface itself is recoverable; the caller receives a detailed `Fail` object and can decide how to proceed (e.g., retry, notify user).

**Panics:**
- **Allowed**: No. As a trait definition intended for a file system abstraction, implementations should return `vfs::Error` rather than panicking.

---

## 6. Traits

- **`RmDir`**: The primary trait defined by this module. It is made `Send` via the `trait_variant` macro.
  - **Method**: `async fn rm_dir(&self, args: Args) -> Result<Success, Fail>;`

---

## 7. Overview

This module is used in order to define the contract for removing directories within the `nfs_mamont` NFSv3 server implementation. It serves as a specific sub-operation of the broader Virtual File System (VFS) layer.

This system contains a set of modular traits (like `RmDir`, `Create`, `Read`) that compose the `Vfs` trait. The `Vfs` trait acts as the primary abstraction between the network protocol handler (RPC) and the underlying storage backend.

A typical usage scenario of the system involves the NFS server receiving an `RMDIR` RPC request from a client. The server decodes the request into `Args` (containing the directory handle and name). It then invokes the `rm_dir` method on the object implementing the `Vfs` trait. The implementation handles the actual storage logic (checking permissions, verifying the directory is empty, unlinking the entry). The result (`Success` or `Fail`) is then encoded back into the NFSv3 wire format, including the `WccData` to ensure the client's cache of the parent directory remains consistent with the server's state.

Inside the system, the following things happen and they use:
- **`vfs::DirOpArgs`**: To precisely locate the target directory entry within the file system hierarchy.
- **`vfs::WccData`**: To maintain cache coherency between the server and the client by providing attributes before and after the modification attempt.
- **`vfs::Error`**: To standardize error reporting across different file system backends, ensuring the client receives accurate NFS status codes.